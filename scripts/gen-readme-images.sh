#!/usr/bin/env bash
# gen-readme-images.sh — tạo bảng so sánh "ảnh gốc vs output qua ae" cho README.
# Dùng imgcat/iterm2 không khả dụng chung → xuất HTML + Markdown side-by-side.
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
B="$here/target/release/ae"
out="$here/docs/images"
mkdir -p "$out"

gen() {
  local src="$1" name="$2" cols="${3:-70}" renderer="${4:-ascii}" extra=()
  # ảnh gốc: copy sang docs/images/
  ext="${src##*.}"
  cp "$src" "$out/${name}-original.${ext}"
  # render qua ae
  if [ "$renderer" = "braille" ]; then
    "$B" render "$src" --renderer braille --dither --width "$cols" > "/tmp/${name}.txt"
  else
    "$B" render "$src" --renderer "$renderer" --width "$cols" > "/tmp/${name}.txt"
  fi
  # convert text → PNG bằng Python PIL (vẽ chữ monospace lên nền)
  python3 - "$out/${name}-ae.png" "/tmp/${name}.txt" <<'PY'
import sys, subprocess
out_path, txt_path = sys.argv[1], sys.argv[2]
code = f'''
from PIL import Image, ImageDraw, ImageFont
lines = open({txt_path!r}).read().splitlines()
fs = 10
font = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", fs) 
try:
    font = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", fs, layout_engine=0)
except Exception:
    try:
        import glob
        f = (glob.glob("/System/Library/Fonts/Menlo*") + glob.glob("/System/Library/Fonts/Supplemental/Menlo*") + glob.glob("/usr/share/fonts/**/*ono*", recursive=True))[0]
        font = ImageFont.truetype(f, fs)
    except Exception:
        font = ImageFont.load_default()
cw = font.getbbox("M")[2] - font.getbbox("M")[0]
lh = fs + 3
W = max((len(l)*cw for l in lines), default=1) + 8
H = len(lines)*lh + 8
img = Image.new("RGB", (W, H), (12,12,16))
draw = ImageDraw.Draw(img)
for i,l in enumerate(lines):
    draw.text((4, 4+i*lh), l, fill=(180,255,140), font=font)
img.save({out_path!r})
print("saved", {out_path!r}, img.size)
'''
subprocess.run(["python3","-c",code], check=True)
PY
}

# Portrait từ ASCII-generator demo/input.jpg
gen ".tmp/ASCII-generator/demo/input.jpg" "portrait" 80

# Complex scene từ demo_image_complex.png — blocks renderer
gen ".tmp/ASCII-generator/demo/demo_image_complex.png" "complex-scene" 90 blocks

# Braille trên chính logo ae
gen "ae_illustration.webp" "logo-braille" 80 braille

echo "done → $out/"
ls -la "$out/"
