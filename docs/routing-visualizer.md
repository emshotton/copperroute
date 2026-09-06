# Routing maze visualizer

The routing visualizer records the maze search as a sequence of SVG images. Each frame overlays
the router's expansion-room geometry on the current PCB state. Capture is opt-in; when it is not
enabled, the normal route output and algorithm are unchanged.

## Capture a route

```bash
cargo run --release -p freerouting -- route board.dsn -o routed.ses \
  --visualize /tmp/board-frames \
  --visualize-every 10 \
  --visualize-max-frames 500
```

Open `/tmp/board-frames/viewer.html` to scrub through or play the captured SVGs. The left/right
arrow keys advance one frame and the space bar starts or stops playback.
Each frame's overlay identifies the current stage (`autorouter` or `optimizer`), pass, event, net,
layer, queue depth, and search costs.

The output directory must be absent or empty, so an accidental rerun cannot overwrite an earlier
capture. The visualizer writes:

- `frame-NNNNNNNN.svg`: one vector image per sampled routing step.
- `frames.jsonl`: frame number, observed step, event, net, layer, and queue depth.
- `viewer.html`: a dependency-free local player.

Colors are:

- blue: traces;
- gold/orange: vias and pins;
- green: complete free-space rooms;
- red: obstacle rooms;
- purple outlines: incomplete free-space rooms;
- yellow: the room currently removed from the maze priority queue;
- magenta: the current door-entry segment.

`route-committed` frames show the PCB after a newly found connection has been inserted. Other
frames are labelled `maze-pop` and show one expansion step.

## Disk-space controls

Capturing every maze expansion on a difficult board can produce hundreds of thousands of images.
The default hard limit is 1,000 frames. Routing continues normally once that limit is reached.

Use `--visualize-every N` to sample one frame from every `N` observed maze steps. Successful route
commits are always retained, so important milestones and the completed routed state are not lost
between samples. A sensible first run for an unfamiliar board is:

```text
--visualize-every 50 --visualize-max-frames 300
```

SVG is used because it preserves exact octagon boundaries and is normally much smaller than a
full-resolution lossless raster frame. `du -sh <frame-directory>` gives the actual capture size.

## Create an MP4 timelapse

The helper uses a temporary raster directory and removes it on exit:

```bash
scripts/routing-timelapse.sh /tmp/board-frames board-routing.mp4 12
```

It requires `ffmpeg` and one SVG rasterizer: `rsvg-convert`, ImageMagick's `magick`, or `inkscape`.
The final argument is frames per second and defaults to 12. Existing video files are not
overwritten.

## Useful options

```text
--visualize DIR
--visualize-every N
--visualize-max-frames N
--visualize-width PIXELS
--visualize-height PIXELS
```

Width and height set the SVG viewport. They do not rasterize the output, so changing them has only
a small effect on capture size.
