import sys
import subprocess
from pathlib import Path

import torch


def cuda_generator(seed: int) -> torch.Generator:
    return torch.Generator(device="cuda").manual_seed(seed)


def torch_allocator_peak_mb() -> int:
    return torch.cuda.max_memory_allocated() // (1 << 20)


def load_int8_text_encoder(repo: str, cls_name: str, revision):
    """bnb int8 T5-class encoder straight onto the GPU (~7.6 GB), transient."""
    import transformers
    cls = getattr(transformers, cls_name)
    return cls.from_pretrained(
        repo, subfolder="text_encoder", revision=revision,
        quantization_config=transformers.BitsAndBytesConfig(load_in_8bit=True),
        device_map="cuda")


def encode_in_subprocess(backend: str, repo: str, revision, prompt: str,
                         negative_prompt: str | None, out_pt: Path,
                         cache_env: dict | None = None) -> float:
    """Run text-encoder prompt embedding in a child process so every byte of
    encoder VRAM (incl. bnb-retained fp32 originals) is released on exit."""
    import os
    import time
    t0 = time.perf_counter()
    child = [
        sys.executable, "-m", "i2v_bench", "_encode-child",
        "--backend", backend, "--repo", repo,
        "--out", str(out_pt),
        "--prompt", prompt,
    ]
    if revision:
        child += ["--revision", revision]
    if negative_prompt:
        child += ["--negative", negative_prompt]
    env = dict(os.environ)
    env.update(cache_env or {})
    r = subprocess.run(child, capture_output=True, text=True, env=env)
    if r.returncode != 0:
        tail = (r.stderr or r.stdout or "").strip().splitlines()[-15:]
        raise RuntimeError("encode-child failed: " + "\n".join(tail))
    return time.perf_counter() - t0


def encode_child_main(args) -> None:
    """Child-process entry: load int8 encoder, encode, torch.save embeds."""
    import torch
    from . import get_backend
    backend = get_backend(args.backend)
    embeds = backend.encode_prompt_embeds(args.repo, args.revision or None,
                                          args.prompt, args.negative)
    torch.save({k: (v.cpu() if isinstance(v, torch.Tensor) else v)
                for k, v in embeds.items()}, args.out)


def free_cuda() -> None:
    """Drop Python garbage and release the CUDA cache. Callers must `del`
    their own references first — `del` on arguments only unbinds locals."""
    import gc
    gc.collect()
    torch.cuda.empty_cache()


def cast_embeds(embeds: dict, dtype: torch.dtype) -> dict:
    """bnb int8 encoders emit fp32/fp16; cast float tensors to the transformer dtype."""
    return {
        k: (v.to(dtype) if isinstance(v, torch.Tensor) and v.is_floating_point() else v)
        for k, v in embeds.items()
    }
