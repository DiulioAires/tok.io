"""Rasterize the logos supplied in public for the Windows widget package."""

from pathlib import Path
from shutil import copyfile

import cairosvg

root = Path(__file__).resolve().parent
assets = root / "Assets"
assets.mkdir(exist_ok=True)

chatgpt = (root.parent / "public" / "Chatgptlogo.svg").read_text(encoding="utf-8")
chatgpt = chatgpt.replace("<svg ", '<svg fill="#f5fff9" ', 1)
cairosvg.svg2png(bytestring=chatgpt.encode(), write_to=str(assets / "chatgpt.png"), output_width=96, output_height=96)

claude = (root.parent / "public" / "Claudelogo.svg").read_bytes()
claude = claude.replace(b'hsl(14.8, 63.1%, 59.6%)', b'#ffa679')
cairosvg.svg2png(bytestring=claude, write_to=str(assets / "claude.png"), output_width=96, output_height=96)

copyfile(root.parent / "src-tauri" / "icons" / "128x128@2x.png", assets / "icon.png")
