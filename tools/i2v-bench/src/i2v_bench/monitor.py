import subprocess
import threading
import time

import psutil

_QUERY = ["nvidia-smi", "--query-gpu=memory.used", "--format=csv,noheader,nounits"]
_CREATIONFLAGS = getattr(subprocess, "CREATE_NO_WINDOW", 0)


def gpu_memory_used_mb() -> int | None:
    try:
        out = subprocess.check_output(_QUERY, text=True, creationflags=_CREATIONFLAGS, timeout=10)
        return int(out.strip().splitlines()[0])
    except Exception:
        return None


class ResourceMonitor:
    """Bounded nvidia-smi + process RSS sampler for generation runs."""

    def __init__(self, interval_s: float = 0.5):
        self.interval_s = interval_s
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self._proc = psutil.Process()
        self.samples: list[int] = []
        self.peak_rss_mb: int = 0
        self.baseline_vram_mb: int | None = None

    def _poll(self) -> None:
        first = True
        while not self._stop.is_set():
            vram = gpu_memory_used_mb()
            if vram is not None:
                if first:
                    self.baseline_vram_mb = vram
                    first = False
                self.samples.append(vram)
            try:
                self.peak_rss_mb = max(self.peak_rss_mb, self._proc.memory_info().rss // (1 << 20))
            except Exception:
                pass
            self._stop.wait(self.interval_s)

    def start(self) -> "ResourceMonitor":
        self._thread = threading.Thread(target=self._poll, daemon=True)
        self._thread.start()
        return self

    def stop(self) -> dict:
        self._stop.set()
        if self._thread:
            self._thread.join(timeout=5)
        return {
            "baseline_vram_mb": self.baseline_vram_mb,
            "peak_vram_mb": max(self.samples) if self.samples else None,
            "peak_process_rss_mb": self.peak_rss_mb or None,
            "vram_sample_count": len(self.samples),
            "vram_sampler": f"nvidia-smi poll {self.interval_s}s (whole-GPU, incl. desktop baseline)",
        }
