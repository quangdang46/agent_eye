#!/usr/bin/env bash
# gen-readme-images.sh — regenerate comparison images for README table
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
B="$here/target/release/ae"
out="$here/docs/images"
mkdir -p "$out"

declare -A SRCS=(
  [portrait]="$here/.tmp/ASCII-generator/demo/input.jpg"
  [button]="$here/crates/ae-render/tests/fixtures/simple-box.png"
  [ui]="$here/crates/ae-core/tests/fixtures/ui.png"
  [logo]="$here/ae_illustration.webp"
)

for name in portrait button ui logo; do
  src="${SRCS[$name]}"
  ext="${src##*.}"
  cp "$src" "$out/${name}-original.${ext}"

  "$B" render "$src" --renderer ascii --width 60 > "/tmp/${name}-ascii.txt"
  "$B" render "$src" --renderer blocks --width 60 > "/tmp/${name}-blocks.txt"
  "$B" render "$src" --renderer braille --dither --width 60 > "/tmp/${name}-braille.txt"
  "$B" geometry "$src" --format json | python3 -c "
import json,sys
d=json.load(sys.stdin)
for r in d['regions']:
    b=r['bounds']
    print(f\"{r['id']} bounds=[{b[0]},{b[1]},{b[2]},{b[3]}] area={r['area']:.3f} edges={r['edge_density']:.2f}\")
print()
kinds={}
for r in d['relations']: kinds[r['type']]=kinds.get(r['type'],0)+1
for k,v in sorted(kinds.items()): print(f'relation: {k} x{v}')
" > "/tmp/${name}-geometry.txt"
done

# Convert text → PNG
python3 << 'INNEREOF'
from PIL import Image, ImageDraw, ImageFont
import glob

def get_font(size=10):
    for p in ["/System/Library/Fonts/Menlo.ttc", "/System/Library/Fonts/Supplemental/Courier New.ttf"]:
        try: return ImageFont.truetype(p, size)
        except: continue
    return ImageFont.load_default()

def text_to_png(txt_path, png_path):
    lines = open(txt_path).read().splitlines()
    font = get_font(9)
    cw = 6; lh = 12
    W = max((len(l)*cw for l in lines), default=60) + 8
    H = len(lines)*lh + 8
    img = Image.new("RGB", (W, H), (12,12,16))
    d = ImageDraw.Draw(img)
    for i, l in enumerate(lines):
        d.text((4, 4+i*lh), l, fill=(160,255,120), font=font)
    img.save(png_path)
    print(f"  {png_path} ({W}x{H})")

for f in sorted(glob.glob("/tmp/*-ascii.txt") + glob.glob("/tmp/*-blocks.txt") +
                glob.glob("/tmp/*-braille.txt") + glob.glob("/tmp/*-geometry.txt")):
    name = f.split("/")[-1].replace(".txt", "")
    text_to_png(f, f"docs/images/{name}.png")
INNEREOF

echo "done"
