"""The argv guard runs (and exits) before `import pcbnew`, so it's testable with plain
python -- no KiCad install needed -- unlike the rest of board_stats.py."""
import subprocess
import sys
from pathlib import Path

SCRIPT = Path(__file__).parent.parent / "vendor" / "kicad" / "board_stats.py"


def test_no_args_exits_2_with_usage():
    r = subprocess.run([sys.executable, str(SCRIPT)], capture_output=True, text=True)
    assert r.returncode == 2
    assert "usage" in r.stderr.lower()


def test_too_many_args_exits_2_with_usage():
    r = subprocess.run([sys.executable, str(SCRIPT), "a.kicad_pcb", "b.kicad_pcb"],
                       capture_output=True, text=True)
    assert r.returncode == 2
    assert "usage" in r.stderr.lower()
