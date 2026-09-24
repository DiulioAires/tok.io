"""Rasterize the logos supplied in public for the Windows widget package."""

from pathlib import Path

import cairosvg
from PIL import Image

root = Path(__file__).resolve().parent
assets = root / "Assets"
assets.mkdir(exist_ok=True)

chatgpt = (root.parent / "public" / "Chatgptlogo.svg").read_text(encoding="utf-8")
chatgpt = chatgpt.replace("<svg ", '<svg fill="#f5fff9" ', 1)
cairosvg.svg2png(bytestring=chatgpt.encode(), write_to=str(assets / "chatgpt.png"), output_width=96, output_height=96)

claude = (root.parent / "public" / "Claudelogo.svg").read_bytes()
claude = claude.replace(b'hsl(14.8, 63.1%, 59.6%)', b'#ffa679')
cairosvg.svg2png(bytestring=claude, write_to=str(assets / "claude.png"), output_width=96, output_height=96)

source = Image.open(root.parent / "public" / "tokio.png").convert("RGBA")
opaque = source.getchannel("A").point(lambda value: 255 if value > 8 else 0)
bounds = opaque.getbbox()
if bounds is None:
    raise ValueError("O logo tokio.png está vazio")
mark = source.crop(bounds)
side = max(mark.size)
padding = round(side * 0.06)
square = Image.new("RGBA", (side + padding * 2, side + padding * 2))
square.alpha_composite(mark, ((square.width - mark.width) // 2, (square.height - mark.height) // 2))
square.resize((1024, 1024), Image.Resampling.LANCZOS).save(root.parent / "public" / "tokio-icon.png")
square.resize((256, 256), Image.Resampling.LANCZOS).save(assets / "icon.png")
