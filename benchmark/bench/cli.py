"""`bench` command-line entry point."""
from __future__ import annotations

import hashlib
import json
import re
import subprocess
from datetime import datetime
from pathlib import Path

import click

from bench import compare as compare_mod
from bench import corpus, export as export_mod, metrics, paths, pool, referee, runner
from bench.candidates import load_candidates, load_referee_java
from bench.report import html as html_report
from bench.report import markdown as md_report
from bench.report import plots as plots_report


def _java_exec(candidates_file: Path | None = None) -> list[str]:
    file = candidates_file or paths.ROOT / "candidates.toml"
    custom = load_referee_java(file) if file.exists() else None
    return custom or [paths.java_exe(), "-Xmx4g", "-jar", str(paths.java_jar())]


def _java_identity(command: list[str]) -> dict:
    identity = {"exec": command, "jar_sha256": None}
    if "-jar" in command:
        index = command.index("-jar") + 1
        if index == len(command) or not command[index] or command[index].startswith("-"):
            raise ValueError("-jar requires a following jar path")
        jar = Path(command[index])
        with jar.open("rb") as stream:
            identity["jar_sha256"] = hashlib.file_digest(stream, "sha256").hexdigest()
    return identity


def _referee_config(boards: list[corpus.Board], candidates_file: Path | None = None,
                    recorded: dict | None = None, verify_recorded: bool = True) -> dict | None:
    if all(b.referee == "kicad" for b in boards):
        return None
    try:
        config = _java_identity(recorded["exec"] if recorded else _java_exec(candidates_file))
        if recorded and verify_recorded and referee.identity_key(config) != referee.identity_key(recorded):
            raise click.ClickException("recorded referee jar has changed; rescore the full run without --only-missing")
        return config
    except (OSError, paths.ToolMissing, ValueError) as e:
        raise click.ClickException(f"Java referee unavailable: {e}") from e


def _score_cell(board: corpus.Board, cell: Path, java_config: dict | None):
    result = referee.score_cell(board, cell, java_config["exec"] if java_config else [])
    if result is not None and board.referee != "kicad":
        result["referee_identity"] = java_config
        (cell / "metrics.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


@click.group()
def main() -> None:
    """Copperroute comparison suite."""


@main.group("corpus")
def corpus_cmd() -> None:
    """Manage the board corpus."""


@corpus_cmd.command("init")
@click.option("--fixtures", type=click.Path(exists=True, path_type=Path),
              default=paths.JAVA_REPO / "fixtures", show_default=True)
def corpus_init(fixtures: Path) -> None:
    """Import the Java repo's DSN fixtures."""
    boards = corpus.init_from_fixtures(fixtures)
    click.echo(f"imported {len(boards)} boards into {corpus.CORPUS / 'dsn'}")


@corpus_cmd.command("list")
@click.option("--tier", default=None)
def corpus_list(tier: str | None) -> None:
    for b in corpus.select(corpus.load_manifest(), tier=tier):
        click.echo(f"{b.id:40} nets={b.nets:4} layers={b.layers:2} referee={b.referee:8} tiers={','.join(b.tiers)}")


@corpus_cmd.command("pcbench")
@click.option("--clone", "root", type=click.Path(path_type=Path), required=True,
              help="PCBench checkout (cloned if missing)")
@click.option("--boards", "max_boards", type=int, default=None)
@click.option("--ids", default=None, help="comma-separated PCBench board dir names")
@click.option("--jobs", default=1, show_default=True,
             help="number of boards to import concurrently (each is an independent KiCad "
                  "subprocess pipeline in its own board directory).")
@click.option("--skip-existing/--force", default=True, show_default=True,
             help="skip boards already imported ok (manifest status ok + unrouted.dsn + "
                  "ground_truth.json on disk) so an interrupted import can resume; "
                  "--force re-imports everything.")
@click.option("--licensed-only", is_flag=True, default=False,
              help="import only boards whose metadata.json records a classified license; "
                   "unlicensed, unclassified and unknown boards are skipped.")
def corpus_pcbench_cmd(root: Path, max_boards, ids, jobs, skip_existing, licensed_only):
    """Import PCBench boards (strip -> DSN -> ground truth)."""
    if jobs < 1:
        raise click.ClickException("--jobs must be >= 1")
    # click.Path doesn't expand `~` itself, and this value often arrives with a literal,
    # unexpanded `~` (e.g. through scripts/remote-corpus.sh, where it must survive several
    # layers of shell quoting to reach the *remote* host's home directory) -- expand it here
    # against whatever process actually runs this command.
    root = root.expanduser()
    from bench import corpus_pcbench
    if not (root / "PCBs").exists():
        click.echo(f"cloning PCBench into {root} ...")
        subprocess.run(["git", "clone", "--depth", "1", "https://github.com/emshotton/PCBench", str(root)], check=True)
    boards = corpus_pcbench.import_boards(root, ids=ids.split(",") if ids else None, max_boards=max_boards,
                                          jobs=jobs, skip_existing=skip_existing, licensed_only=licensed_only,
                                          progress=click.echo)
    ok = sum(1 for b in boards if b.status == "ok")
    click.echo(f"imported {ok}/{len(boards)} boards")
    for b in boards:
        if b.status != "ok":
            click.echo(f"  excluded {b.id}: {b.reason}")


@corpus_cmd.command("kicad-fixtures")
@click.option("--fixtures", type=click.Path(exists=True, path_type=Path),
              default=paths.JAVA_REPO / "fixtures", show_default=True)
@click.option("--jobs", default=1, show_default=True,
             help="number of boards to import concurrently (each is an independent KiCad "
                  "subprocess pipeline in its own board directory).")
@click.option("--skip-existing/--force", default=True, show_default=True,
             help="skip boards already imported ok (manifest status ok + unrouted.dsn + "
                  "ground_truth.json on disk) so an interrupted import can resume; "
                  "--force re-imports everything.")
def corpus_kicad_fixtures_cmd(fixtures: Path, jobs: int, skip_existing: bool) -> None:
    """Import the Java repo's own KiCad fixture boards (strip -> DSN -> ground truth)."""
    if jobs < 1:
        raise click.ClickException("--jobs must be >= 1")
    from bench import corpus_pcbench
    boards = corpus_pcbench.import_kicad_fixtures(fixtures, jobs=jobs, skip_existing=skip_existing,
                                                   progress=click.echo)
    ok = sum(1 for b in boards if b.status == "ok")
    click.echo(f"imported {ok}/{len(boards)} boards")
    for b in boards:
        if b.status != "ok":
            click.echo(f"  excluded {b.id}: {b.reason}")


@corpus_cmd.command("revalidate")
@click.option("--origin", type=click.Choice(["pcbench", "freerouting-kicad"]), default="pcbench",
             show_default=True)
@click.option("--regenerate-projects", is_flag=True,
             help="regenerate raw.kicad_pro/stripped.kicad_pro from raw.orig.kicad_pcb for "
                  "boards whose project was bench-generated (ground_truth.project_generated), "
                  "so a vendor/kicad/legacy_rules.py fix is picked up without a full re-import.")
@click.option("--rerun-drc", is_flag=True,
             help="re-run kicad-cli pcb drc on raw.kicad_pcb (against the possibly "
                  "just-regenerated project) and rewrite raw-drc.json before recomputing; "
                  "needs kicad-cli. Without this flag, only boards with an existing "
                  "raw-drc.json are touched and no KiCad install is needed at all.")
@click.option("--jobs", default=1, show_default=True,
             help="boards to revalidate concurrently; only matters with --rerun-drc, since the "
                  "plain recompute is pure local file I/O.")
def corpus_revalidate_cmd(origin: str, regenerate_projects: bool, rerun_drc: bool, jobs: int) -> None:
    """Recompute drv_routing/drv_all/unconnected/reference_complete (and, optionally, the
    generated project and/or DRC report) for every already-imported --origin board from what's
    on disk, and re-apply the reference-board exclusion rule to the manifest -- without
    re-running the full strip -> DSN -> DRC -> stats import pipeline."""
    if jobs < 1:
        raise click.ClickException("--jobs must be >= 1")
    from bench import corpus_pcbench
    try:
        summary = corpus_pcbench.revalidate(origin, progress=click.echo,
                                            regenerate_projects=regenerate_projects,
                                            rerun_drc=rerun_drc, jobs=jobs)
    except paths.ToolMissing as e:
        raise click.ClickException(str(e)) from e
    click.echo(f"checked {summary['checked']} {origin} board(s), {summary['skipped']} skipped"
              + (f", {summary['regenerated_projects']} project(s) regenerated" if regenerate_projects else "")
              + (f", {summary['reran_drc']} DRC re-run(s)" if rerun_drc else ""))
    if summary["ok_to_excluded"]:
        by_reason: dict[str, int] = {}
        for _bid, reason in summary["ok_to_excluded"]:
            key = re.sub(r"\d+", "N", reason)
            by_reason[key] = by_reason.get(key, 0) + 1
        click.echo(f"ok -> excluded: {len(summary['ok_to_excluded'])}")
        for reason, n in sorted(by_reason.items()):
            click.echo(f"  {n}x {reason}")
    if summary["excluded_to_ok"]:
        click.echo(f"excluded -> ok: {len(summary['excluded_to_ok'])}")
        for bid in summary["excluded_to_ok"]:
            click.echo(f"  {bid}")


@corpus_cmd.command("connections")
@click.option("--tier", default=None)
@click.option("--boards", "board_ids", default=None, help="comma-separated board ids")
def corpus_connections_cmd(tier: str | None, board_ids: str | None) -> None:
    """Measure each board's Java connection count (board_statistics.connections.maximum_count)
    and store it in the manifest as `connections`, the N used by metrics.score when a
    candidate's own self-report is unavailable."""
    from bench.referee import java_drc
    all_boards = corpus.load_manifest()
    try:
        selected = corpus.select(all_boards, tier=tier, ids=board_ids.split(",") if board_ids else None)
    except KeyError as e:
        raise click.ClickException(e.args[0]) from e
    by_id = {b.id: b for b in all_boards}
    java_exec = _java_exec()
    for b in selected:
        n = java_drc.measure_connections(b, java_exec)
        by_id[b.id].connections = n
        click.echo(f"{b.id}: connections={n}" if n is not None else f"{b.id}: FAILED to measure connections")
        # save after every board so a long run's progress survives an interruption
        corpus.save_manifest(list(by_id.values()))


@main.command("run")
@click.option("--candidates", "cand_names", required=True, help="comma-separated names from candidates.toml")
@click.option("--candidates-file", "candidates_file", envvar="BENCH_CANDIDATES",
             type=click.Path(exists=True, path_type=Path), default=None,
             help="candidates TOML file to load --candidates from (also settable via $BENCH_CANDIDATES; "
                  "default: candidates.toml next to this repo's pyproject.toml); relative exec paths in "
                  "it resolve relative to the file's own directory. Used by scripts/remote-run.sh to "
                  "point at candidates.remote.toml on hosts without the Java repo checkout.")
@click.option("--tier", default=None)
@click.option("--boards", "board_ids", default=None, help="comma-separated board ids")
@click.option("--seeds", type=click.IntRange(min=1), default=1, show_default=True,
              help="repetitions per board; {seed} in candidate extra_args receives the repetition index")
@click.option("--max-passes", default=100, show_default=True)
@click.option("--timeout", "timeout_s", default=300, show_default=True)
@click.option("--threads", type=click.IntRange(min=1), default=1, show_default=True)
@click.option("--jobs", default=1, show_default=True,
             help="number of cells (candidate x board x seed) to run concurrently. "
                  "Use --jobs 1 for controlled quality and "
                  "wall-time numbers -- with --jobs > 1, `compare` falls back to CPU time "
                  "since wall time is contended. Rule of thumb (~1.5 GB RSS per Java cell): "
                  "4 on a 10-core/16 GB laptop, 8-12 on a 16-core/64 GB workstation.")
@click.option("--run-id", default=None)
@click.option("--no-referee", is_flag=True)
def run_cmd(cand_names, candidates_file, tier, board_ids, seeds, max_passes, timeout_s, threads, jobs,
           run_id, no_referee):
    """Route boards with candidates and score the results."""
    if jobs < 1:
        raise click.ClickException("--jobs must be >= 1")
    candidates_file = Path(candidates_file) if candidates_file else paths.ROOT / "candidates.toml"
    try:
        chosen = list(load_candidates(candidates_file, cand_names.split(",")).values())
    except KeyError as e:
        raise click.ClickException(f"unknown candidate: {e.args[0]}") from e
    except (FileNotFoundError, paths.ToolMissing, ValueError) as e:
        raise click.ClickException(str(e)) from e
    try:
        boards = corpus.select(corpus.load_manifest(), tier=tier,
                               ids=board_ids.split(",") if board_ids else None)
    except KeyError as e:
        raise click.ClickException(e.args[0]) from e
    if not boards:
        raise click.ClickException("no boards selected")
    missing = [b.id for b in boards if not b.path.exists()]
    if missing:
        raise click.ClickException(
            "missing board input file(s) for: " + ", ".join(missing)
            + " -- run `bench corpus init` / `bench corpus kicad-fixtures` (or `bench corpus pcbench`)")
    run_id = run_id or datetime.now().strftime("%Y%m%d-%H%M%S")
    cfg = runner.RunConfig(run_id=run_id, candidates=chosen, boards=boards, seeds=seeds,
                           max_passes=max_passes, timeout_s=timeout_s, threads=threads, jobs=jobs, tier=tier,
                           candidates_file=str(candidates_file.resolve()))
    hook = None
    if not no_referee:
        cfg.referee_java = _referee_config(boards, candidates_file)
        hook = lambda board, cell: _score_cell(board, cell, cfg.referee_java)
    try:
        run_dir = runner.run(cfg, referee=hook, progress=click.echo)
    except FileExistsError as e:
        raise click.ClickException(f"run already exists: {run_id}; choose a new --run-id") from e
    click.echo(f"run written to {run_dir}")


@main.command("referee")
@click.option("--run", "run_id", required=True)
@click.option("--only-missing", is_flag=True, help="skip cells that already have metrics.json")
@click.option("--candidates-file", type=click.Path(exists=True, path_type=Path), default=None,
              help="referee configuration override; changing the referee requires rescoring the full run")
@click.option("--jobs", default=1, show_default=True,
             help="number of cells to re-score concurrently. Needs only the run's manifest and "
                  "results -- no candidates.toml.")
def referee_cmd(run_id: str, only_missing: bool, jobs: int, candidates_file: Path | None) -> None:
    """(Re)score every cell of a run."""
    if jobs < 1:
        raise click.ClickException("--jobs must be >= 1")
    run_dir = runner.RESULTS / run_id
    meta = runner.load_meta(run_dir)
    boards = {b.id: b for b in corpus.load_manifest()}
    todo = []
    for c in meta["cells"]:
        cell = runner.cell_dir(run_dir, c["candidate"], c["board"], c["seed"])
        if only_missing and (cell / "metrics.json").exists():
            continue
        todo.append((c, cell))

    if only_missing and meta.get("referee_rescore", {}).get("status") == "incomplete":
        raise click.ClickException("full rescore is incomplete; rerun without --only-missing")
    if not todo:
        click.echo("scored 0 cells: nothing left to score")
        return

    selected_boards = [boards[c["board"]] for c, _ in todo]
    recorded = meta.get("referee_java")
    source = candidates_file or (Path(meta["candidates_file"]) if meta.get("candidates_file") else None)
    java_config = _referee_config(selected_boards, source, recorded if not candidates_file else None,
                                  verify_recorded=only_missing)
    if java_config is not None and only_missing and recorded and (
            referee.identity_key(java_config) != referee.identity_key(recorded)):
        raise click.ClickException("cannot change the referee with --only-missing; rescore the full run")
    if not only_missing:
        meta["referee_rescore"] = {"status": "incomplete", "referee_java": java_config}
        runner._save_meta(run_dir, meta)

    def _score(item: tuple[dict, Path]) -> tuple[dict, dict, str | None]:
        c, cell = item
        board = boards[c["board"]]
        try:
            return c, _score_cell(board, cell, java_config), None
        except Exception as e:
            # One bad cell must not abort the rest of the pass -- especially under jobs > 1,
            # where other cells may already be scoring concurrently in other worker threads.
            # Same fallback the runner uses for a referee hook that raises.
            reason = f"referee raised {type(e).__name__}: {e}"
            r = {"status": "referee_failed", "referee": board.referee, "reason": reason,
                 "candidate_output": (cell / "out.ses").exists()}
            (cell / "referee.json").write_text(json.dumps(r, indent=2) + "\n")
            return c, metrics.build(cell, board), reason

    def _print(result: tuple[dict, dict, str | None]) -> None:
        c, m, error = result
        if error:
            click.echo(f"{c['candidate']:16} {c['board']:32} seed {c['seed']}: ERROR {error}")
        else:
            click.echo(f"{c['candidate']:16} {c['board']:32} seed {c['seed']}: "
                       f"unrouted={m['unrouted']} viol={m['violations']} score={m['score']:.1f}"
                       + (" DISAGREE" if m["disagreement"] else "") + (" FAILED" if m["failed"] else ""))

    # Results are printed (via on_done) in submission order from the main thread, same
    # pattern as `run` uses for meta.json -- regardless of which worker finishes first.
    results = pool.run_cells(todo, _score, jobs, on_done=_print)

    if java_config is not None:
        meta["referee_java"] = java_config
    if not only_missing:
        meta["referee_rescore"]["status"] = "complete"
    runner._save_meta(run_dir, meta)

    scored = len(results)
    clean = sum(1 for _, m, _ in results if m["clean_pass"])
    failed = sum(1 for _, m, error in results if error or m["failed"])
    disagree = sum(1 for _, m, _ in results if m["disagreement"])
    click.echo(f"scored {scored} cells: {clean} clean pass, {failed} failed, {disagree} disagreements")


def _latest_run_for(name: str) -> Path:
    runs = sorted(runner.RESULTS.glob("*/meta.json"), key=lambda p: p.stat().st_mtime, reverse=True)
    for m in runs:
        if any(c["name"] == name for c in json.loads(m.read_text())["candidates"]):
            return m.parent
    raise click.ClickException(f"no run contains candidate {name}")


@main.command("compare")
@click.option("--baseline", required=True)
@click.option("--against", required=True, help="comma-separated candidate names")
@click.option("--runs", default=None, help="comma-separated run ids (default: latest run per candidate)")
@click.option("--tier", default=None)
@click.option("--out", "out_name", default=None)
@click.option("--allow-mixed", is_flag=True)
@click.option("--fail-on-regression", is_flag=True,
              help="exit nonzero for routing-quality losses or no comparable boards; timing/RSS are advisory by default")
@click.option("--require-complete", is_flag=True,
              help="with --fail-on-regression, also require every shared board and planned repetition")
@click.option("--performance-regression-percent", type=click.FloatRange(min=0, min_open=True), default=None,
              help="opt in to time/RSS gating above this percent and a noise allowance; "
                   "with --fail-on-regression, fewer than 3 measured samples per side also fails the gate")
@click.option("--time-metric", "time_metric", type=click.Choice(["auto", "wall", "cpu"]), default="auto",
             show_default=True,
             help="which time metric decides verdicts and median_time_ratio. 'auto' uses wall time "
                  "when every compared run used --jobs 1, else falls back to CPU time (wall time is "
                  "contended once cells run concurrently).")
def compare_cmd(baseline, against, runs, tier, out_name, allow_mixed, fail_on_regression, require_complete, performance_regression_percent, time_metric):
    """Compare candidates and write reports/<id>.{json,md,html}."""
    names = against.split(",")
    if runs:
        ids = runs.split(",")
        missing = [r for r in ids if not (runner.RESULTS / r / "meta.json").exists()]
        if missing:
            raise click.ClickException(f"unknown run id(s): {', '.join(missing)}")
        run_dirs = [runner.RESULTS / r for r in ids]
    else:
        run_dirs = []
        for n in [baseline, *names]:
            rd = _latest_run_for(n)
            if rd not in run_dirs:
                run_dirs.append(rd)
    try:
        cmp = compare_mod.compare(run_dirs, baseline, names, corpus.load_manifest(), tier=tier,
                                  allow_mixed=allow_mixed, time_metric=time_metric,
                                  performance_regression_percent=performance_regression_percent)
    except compare_mod.IncompatibleRuns as e:
        raise click.ClickException(str(e)) from e
    out_name = out_name or f"{datetime.now().strftime('%Y%m%d-%H%M%S')}-{baseline}-vs-{'-'.join(names)}"
    paths.REPORTS.mkdir(exist_ok=True)
    (paths.REPORTS / f"{out_name}.json").write_text(json.dumps(cmp, indent=2) + "\n")
    (paths.REPORTS / f"{out_name}.md").write_text(md_report.render(cmp))
    (paths.REPORTS / f"{out_name}.html").write_text(html_report.render(cmp, html_report.load_history(paths.REPORTS)))
    for n in names:
        click.echo(f"{n} vs {baseline}: {cmp['overall'][n]['verdict']} "
                   f"({cmp['overall'][n]['wins']}W/{cmp['overall'][n]['losses']}L/{cmp['overall'][n]['ties']}T)")
    for name, cov in cmp["coverage"].items():
        click.echo(f"{name}: compared {cov['compared_boards']}/{len(cov['shared_boards'])} shared boards; "
                   f"{len(cov['incomplete_boards'])} incomplete, {len(cov['skipped_boards'])} skipped")
    click.echo(f"reports written to {paths.REPORTS / out_name}.{{json,md,html}}")
    if fail_on_regression:
        rejected = any(o["quality_losses"] or o["performance_losses"] or o["performance_unmeasured"]
                       or o["verdict"] == "inconclusive" for o in cmp["overall"].values())
        incomplete = any(c["incomplete_boards"] or c["skipped_boards"] for c in cmp["coverage"].values())
        if rejected or (require_complete and incomplete):
            raise click.ClickException("regression check failed; see the comparison report")



@main.command("report")
@click.option("--compare", "compare_id", required=True)
def report_cmd(compare_id: str) -> None:
    """Regenerate Markdown and HTML from a compare JSON."""
    cmp = json.loads((paths.REPORTS / f"{compare_id}.json").read_text())
    (paths.REPORTS / f"{compare_id}.md").write_text(md_report.render(cmp))
    (paths.REPORTS / f"{compare_id}.html").write_text(html_report.render(cmp, html_report.load_history(paths.REPORTS)))
    click.echo(f"wrote {paths.REPORTS / compare_id}.{{md,html}}")


@main.command("export")
@click.option("--run", "run_id", required=True)
@click.option("--candidate", "candidate", required=True)
@click.option("--out-dir", "out_dir", type=click.Path(path_type=Path), default=None,
             help="default: <repo root>/exports (git-tracked, unlike results/ and reports/)")
def export_cmd(run_id: str, candidate: str, out_dir: Path | None) -> None:
    """Export one candidate's per-board metrics from a run to exports/<run>-<candidate>-<sha7>.{csv,json}."""
    run_dir = runner.RESULTS / run_id
    if not (run_dir / "meta.json").exists():
        raise click.ClickException(f"unknown run: {run_id}")
    out_dir = out_dir or (paths.ROOT / "exports")
    boards = corpus.load_manifest()
    try:
        result = export_mod.write_export(run_dir, candidate, boards, out_dir)
    except KeyError as e:
        raise click.ClickException(f"unknown candidate: {e.args[0]}") from e
    if result["warning"]:
        click.echo(f"warning: {result['warning']}")
    msg = f"wrote {result['csv_path']} and {result['json_path']} ({len(result['rows'])} board(s)"
    if result["skipped"]:
        msg += f", {result['skipped']} skipped (excluded from corpus manifest)"
    click.echo(msg + ")")


@main.command("plot")
@click.option("--files", "files_csv", required=True,
             help="comma-separated `bench export` JSON file paths")
@click.option("--out", "out_path", type=click.Path(path_type=Path), default=None,
             help="default: reports/plot.html")
def plot_cmd(files_csv: str, out_path: Path | None) -> None:
    """Render a self-contained HTML page of scatter/bar plots from `bench export` JSON files."""
    file_paths = [Path(p) for p in files_csv.split(",")]
    missing = [str(p) for p in file_paths if not p.exists()]
    if missing:
        raise click.ClickException(f"missing export file(s): {', '.join(missing)}")
    try:
        exports = [plots_report.load_export(p) for p in file_paths]
    except ValueError as e:
        raise click.ClickException(f"invalid export JSON: {e}") from e
    page = plots_report.render(exports)
    out_path = out_path or (paths.REPORTS / "plot.html")
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(page)
    click.echo(f"wrote {out_path}")


if __name__ == "__main__":
    main()
