# gb-hardware_GB-BRK-M-XS

- Source: https://github.com/Gekkio/gb-hardware
- Source commit: b5ade559bd988fd6c4f5941615d4d6d79dec9098
- Author: Gekkio
- License: CC-BY-4.0 (Creative Commons Attribution 4.0 International)
- License text: `LICENSE.txt` in this directory, copied verbatim from the source repository

## Files in this directory

`gb-breakout.kicad_pcb` is the board from the source at that commit, cleaned by the PCBench
scripts under `Scripts/Data_cleaning`. `gb-breakout.kicad_pro` is an unmodified copy of the project file from the same commit, and supplies the board's net classes. Only the filenames are shortened for
distribution. Both are provided under the same license as the source.

Downloads produced by the demo include a dated modification notice in the board's title
block. The router replaces tracks and vias and clears zone fill caches; users must refill
zones and verify the result in KiCad.
