from pathlib import Path

from PIL import Image

from .util import sha256_file, write_json


def prepare_input(src: Path, canvas_w: int, canvas_h: int, margin_frac: float, out: Path) -> dict:
    """Deterministic letterbox preprocessing: sRGB RGB on white, aspect preserved, centered."""
    img = Image.open(src)
    if img.mode in ("RGBA", "LA", "P"):
        img = img.convert("RGBA")
        bg = Image.new("RGB", img.size, (255, 255, 255))
        bg.paste(img, mask=img.split()[-1])
        img = bg
    else:
        img = img.convert("RGB")

    inner_w = int(canvas_w * (1 - 2 * margin_frac))
    inner_h = int(canvas_h * (1 - 2 * margin_frac))
    scale = min(inner_w / img.width, inner_h / img.height)
    new_w = max(1, round(img.width * scale))
    new_h = max(1, round(img.height * scale))
    img = img.resize((new_w, new_h), Image.LANCZOS)

    canvas = Image.new("RGB", (canvas_w, canvas_h), (255, 255, 255))
    canvas.paste(img, ((canvas_w - new_w) // 2, (canvas_h - new_h) // 2))

    out.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out, "PNG")

    record = {
        "source_path": str(src),
        "source_sha256": sha256_file(src),
        "derived_path": str(out),
        "derived_sha256": sha256_file(out),
        "canvas": [canvas_w, canvas_h],
        "margin_frac": margin_frac,
        "placed_size": [new_w, new_h],
        "background": "white",
        "source_mode": Image.open(src).mode,
        "source_size": list(Image.open(src).size),
    }
    write_json(out.with_suffix(".prep.json"), record)
    return record
