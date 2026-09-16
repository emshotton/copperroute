set -euo pipefail
OUT=/home/em/copperroute-after-repair-validation
trap 'code=$?; printf "%s\n" "$code" > "$OUT/driver.exit"' EXIT
cd /home/em/copperroute-after-optimization-repair
cargo test --workspace -j 8 > "$OUT/workspace.log" 2>&1
cargo build --release --locked -p copperroute -j 12 > "$OUT/build.log" 2>&1
cd /home/em/copperroute-congestion-main/benchmark
export COPPERROUTE_TIME=/run/current-system/sw/bin/time
export COPPERROUTE_KICAD_CLI=/run/current-system/sw/bin/kicad-cli
export COPPERROUTE_KICAD_PYTHON=/home/em/freerouting-bench-env/kicad-python
export UV_PYTHON_DOWNLOADS=never
uv run bench run --candidates repair --candidates-file "$OUT/candidates.toml" --boards "$(cat "$OUT/boards.txt")" --max-passes 10 --timeout 300 --threads 1 --jobs 12 --run-id after-repair-cleanup-01
for arm in repair; do
uv run bench export --run after-repair-cleanup-01 --candidate "$arm"
done
