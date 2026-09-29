import platform
import socket
import subprocess

_CREATIONFLAGS = getattr(subprocess, "CREATE_NO_WINDOW", 0)


def gpu_identity() -> dict:
    try:
        out = subprocess.check_output(
            ["nvidia-smi", "--query-gpu=name,driver_version,memory.total",
             "--format=csv,noheader"],
            text=True, creationflags=_CREATIONFLAGS, timeout=10,
        ).strip().splitlines()[0]
        name, driver, mem = [p.strip() for p in out.split(",")]
        return {"gpu_name": name, "driver_version": driver, "gpu_memory_total": mem}
    except Exception as e:
        return {"gpu_name": "unknown", "driver_version": "unknown", "error": str(e)}


def env_identity() -> dict:
    import torch
    ident = {
        "hostname": socket.gethostname(),
        "os": platform.platform(),
        "python": platform.python_version(),
        "torch": torch.__version__,
        "torch_cuda": torch.version.cuda,
        "cuda_available": torch.cuda.is_available(),
    }
    ident.update(gpu_identity())
    try:
        import diffusers
        ident["diffusers"] = diffusers.__version__
    except Exception:
        pass
    try:
        import transformers
        ident["transformers"] = transformers.__version__
    except Exception:
        pass
    return ident
