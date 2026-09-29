import subprocess
from pathlib import Path

from PIL import Image

_CREATIONFLAGS = getattr(subprocess, "CREATE_NO_WINDOW", 0)


def save_frames(frames: list, frames_dir: Path) -> list[Path]:
    frames_dir.mkdir(parents=True, exist_ok=True)
    paths = []
    for i, f in enumerate(frames):
        p = frames_dir / f"f{i:05d}.png"
        f.save(p, "PNG")
        paths.append(p)
    return paths


def encode_mp4(frames_dir: Path, fps: int, out_mp4: Path) -> None:
    out_mp4.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        "ffmpeg", "-y", "-framerate", str(fps),
        "-i", str(frames_dir / "f%05d.png"),
        "-c:v", "libx264", "-preset", "medium", "-crf", "18",
        "-pix_fmt", "yuv420p", "-movflags", "+faststart",
        str(out_mp4),
    ]
    r = subprocess.run(cmd, capture_output=True, text=True, creationflags=_CREATIONFLAGS)
    if r.returncode != 0:
        raise RuntimeError(f"ffmpeg encode failed: {r.stderr[-2000:]}")


def probe_mp4(path: Path) -> dict:
    r = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "v:0",
         "-show_entries", "stream=width,height,nb_frames,avg_frame_rate",
         "-of", "json", str(path)],
        capture_output=True, text=True, creationflags=_CREATIONFLAGS,
    )
    if r.returncode != 0:
        raise RuntimeError(f"ffprobe failed: {r.stderr[-1000:]}")
    import json
    return json.loads(r.stdout)["streams"][0]


def representative_indices(n: int, tiles: int = 9) -> list[int]:
    if n <= tiles:
        return list(range(n))
    return sorted({round(i * (n - 1) / (tiles - 1)) for i in range(tiles)})


def contact_sheet(frame_paths: list[Path], indices: list[int], out: Path,
                  thumb_w: int = 240, cols: int = 3) -> None:
    thumbs = []
    for i in indices:
        im = Image.open(frame_paths[i])
        h = round(im.height * thumb_w / im.width)
        im = im.resize((thumb_w, h), Image.LANCZOS)
        thumbs.append((i, im))
    rows = (len(thumbs) + cols - 1) // cols
    cw = max(t.width for _, t in thumbs)
    ch = max(t.height for _, t in thumbs)
    label_h = 18
    sheet = Image.new("RGB", (cols * cw, rows * (ch + label_h)), (32, 32, 32))
    from PIL import ImageDraw
    draw = ImageDraw.Draw(sheet)
    for k, (idx, t) in enumerate(thumbs):
        x = (k % cols) * cw
        y = (k // cols) * (ch + label_h)
        sheet.paste(t, (x, y))
        draw.text((x + 4, y + ch + 2), f"frame {idx}", fill=(220, 220, 220))
    out.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(out, "PNG")
