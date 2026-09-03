"""A stand-in router for tests. Behaviour is chosen by FAKE_MODE env var:
ok (default) | crash | hang | misreport | no_output
Accepts the Java legacy flags and writes out.ses + result.json like a real candidate."""
import json
import os
import sys
import time


def main() -> int:
    args = sys.argv[1:]
    get = lambda flag: args[args.index(flag) + 1]
    out_ses = get("-do")
    result_json = next(a.split("=", 1)[1] for a in args if a.startswith("--router.result_json="))
    mode = os.environ.get("FAKE_MODE", "ok")
    if mode == "hang":
        time.sleep(3600)
    if mode == "crash":
        print("boom", file=sys.stderr)
        return 3
    if mode == "no_output":
        return 0
    with open(out_ses, "w") as f:
        f.write("(session fake (routes (network (net A (via VIA 1 2) (wire (path F.Cu 250 0 0 1000 0))))))\n")
    unrouted = 5 if mode == "misreport" else 0
    manifest = {
        "schema_version": 1, "app_version": "fake-1.0", "git_sha": "fake",
        "fake_env": {"HOME": os.environ.get("HOME"), "XDG_CONFIG_HOME": os.environ.get("XDG_CONFIG_HOME")},
        "phases": {"fanout": {"duration_seconds": 0.0, "passes_completed": 0},
                   "autorouter": {"duration_seconds": 0.5, "passes_completed": 3},
                   "optimizer": {"duration_seconds": 0.1, "passes_completed": 1}},
        "board_statistics": {
            "connections": {"maximum_count": 10, "incomplete_count": unrouted},
            "traces": {"total_count": 8, "total_length_mm": 123.4},
            "vias": {"total_count": 4},
            "bends": {"total_count": 6},
            "clearance_violations": {"total_count": 0},
        },
        "normalized_score": 990.0, "final_state": "COMPLETED", "exit_code": 0, "output_written": True,
    }
    with open(result_json, "w") as f:
        json.dump(manifest, f)
    return 0


if __name__ == "__main__":
    sys.exit(main())
