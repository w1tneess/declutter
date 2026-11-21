"""Regenerate src-tauri/icons from assets/icon.ico (run from the repo root: python src-tauri/gen_icons.py).

Same output as `tauri icon` for a Windows-only bundle: a multi-size icon.ico plus the PNG set tauri.conf.json lists.
Needs Pillow (requirements-dev.txt).
"""
from pathlib import Path

from PIL import Image

src = Image.open("assets/icon.ico")
src.size = max(src.info["sizes"])  # the largest frame (256 px)
img = src.convert("RGBA")
out = Path("src-tauri/icons")
out.mkdir(parents=True, exist_ok=True)
img.save(out / "icon.ico", sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
for name, size in {"32x32.png": 32, "128x128.png": 128, "128x128@2x.png": 256, "icon.png": 512}.items():
    img.resize((size, size), Image.LANCZOS).save(out / name)
