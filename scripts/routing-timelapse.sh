#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 || $# -gt 3 ]]; then
  echo "usage: $0 FRAME_DIRECTORY OUTPUT.mp4 [FPS]" >&2
  exit 2
fi

frame_directory=$1
video_output=$2
frames_per_second=${3:-12}

if [[ ! -d "$frame_directory" ]]; then
  echo "frame directory does not exist: $frame_directory" >&2
  exit 2
fi
if [[ -e "$video_output" ]]; then
  echo "refusing to overwrite existing output: $video_output" >&2
  exit 2
fi
if ! [[ "$frames_per_second" =~ ^[1-9][0-9]*$ ]]; then
  echo "FPS must be a positive integer" >&2
  exit 2
fi
if ! command -v ffmpeg >/dev/null 2>&1; then
  echo "ffmpeg is required" >&2
  exit 1
fi

if command -v rsvg-convert >/dev/null 2>&1; then
  rasterizer=rsvg
elif command -v magick >/dev/null 2>&1; then
  rasterizer=magick
elif command -v inkscape >/dev/null 2>&1; then
  rasterizer=inkscape
else
  echo "install rsvg-convert, ImageMagick, or Inkscape to rasterize SVG frames" >&2
  exit 1
fi

temporary_frames=$(mktemp -d)
cleanup() {
  rm -rf -- "$temporary_frames"
}
trap cleanup EXIT

frame_count=0
while IFS= read -r source_frame; do
  frame_count=$((frame_count + 1))
  destination_frame=$(printf '%s/frame-%08d.png' "$temporary_frames" "$frame_count")
  case "$rasterizer" in
    rsvg) rsvg-convert "$source_frame" -o "$destination_frame" ;;
    magick) magick "$source_frame" "$destination_frame" ;;
    inkscape) inkscape "$source_frame" --export-filename="$destination_frame" >/dev/null ;;
  esac
done < <(find "$frame_directory" -maxdepth 1 -type f -name 'frame-*.svg' -print | LC_ALL=C sort)

if [[ $frame_count -eq 0 ]]; then
  echo "no frame-*.svg files found in $frame_directory" >&2
  exit 1
fi

ffmpeg -hide_banner -loglevel warning -n \
  -framerate "$frames_per_second" \
  -i "$temporary_frames/frame-%08d.png" \
  -c:v libx264 -pix_fmt yuv420p -movflags +faststart \
  "$video_output"

echo "wrote $video_output from $frame_count frames"
