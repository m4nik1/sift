#!/bin/sh
# Builds assets/AppIcon.icns from icon-draft/icon.svg.
# The artwork is inset on the 1024 canvas (about 8% each side), as macOS app icons are.
set -e
cd "$(dirname "$0")"
work=$(mktemp -d)
rsvg-convert -w 864 -h 864 icon-draft/icon.svg -o "$work/art.png"
python3 - "$work/art.png" "$work/master.png" <<'PY'
import sys
from PIL import Image
art = Image.open(sys.argv[1]).convert("RGBA")
canvas = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
canvas.paste(art, ((1024 - art.width) // 2, (1024 - art.height) // 2), art)
canvas.save(sys.argv[2])
PY
set_dir="$work/AppIcon.iconset"
mkdir "$set_dir"
for s in 16 32 128 256 512; do
  sips -z $s $s "$work/master.png" --out "$set_dir/icon_${s}x${s}.png" >/dev/null
  d=$((s * 2))
  sips -z $d $d "$work/master.png" --out "$set_dir/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$set_dir" -o AppIcon.icns
cp "$work/master.png" AppIcon-1024.png
rm -rf "$work"
echo "wrote $(pwd)/AppIcon.icns"
