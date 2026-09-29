import torch
from diffusers import (AutoencoderKLLTXVideo, FlowMatchEulerDiscreteScheduler,
                       LTXConditionPipeline, LTXVideoTransformer3DModel)
from transformers import T5Tokenizer

from .common import (cast_embeds, cuda_generator, encode_in_subprocess,
                     load_int8_text_encoder)

BACKEND = "ltx"


def encode_prompt_embeds(repo: str, revision, prompt: str,
                         negative_prompt: str | None) -> dict:
    """Runs inside the encode child process; returns CPU-freeable tensors."""
    tok = T5Tokenizer.from_pretrained(repo, subfolder="tokenizer", revision=revision)
    te = load_int8_text_encoder(repo, "T5EncoderModel", revision)
    enc_pipe = LTXConditionPipeline(scheduler=None, vae=None, text_encoder=te,
                                    tokenizer=tok, transformer=None)
    res = enc_pipe.encode_prompt(
        prompt=prompt, negative_prompt=negative_prompt,
        do_classifier_free_guidance=negative_prompt is not None,
        device="cuda", max_sequence_length=256)
    embeds = cast_embeds(
        dict(zip(("prompt_embeds", "prompt_attention_mask",
                  "negative_prompt_embeds", "negative_prompt_attention_mask"), res)),
        torch.bfloat16)
    embeds["_meta"] = {
        "prompt_tokens": len(tok(prompt, truncation=False).input_ids),
        "prompt_truncated": len(tok(prompt, truncation=False).input_ids) > 256,
        "max_sequence_length": 256,
    }
    return embeds


def encode(profile: dict, prompt: str, negative_prompt: str | None,
           out_pt) -> float:
    return encode_in_subprocess(BACKEND, profile["hf_repo"],
                                profile.get("hf_revision") or None,
                                prompt, negative_prompt, out_pt)


def load_pipe(profile: dict) -> dict:
    """Generation components only — text encoder is never touched."""
    repo = profile["hf_repo"]
    rev = profile.get("hf_revision") or None
    transformer = LTXVideoTransformer3DModel.from_pretrained(
        repo, subfolder="transformer", revision=rev,
        torch_dtype=torch.bfloat16, device_map="cuda")
    vae = AutoencoderKLLTXVideo.from_pretrained(
        repo, subfolder="vae", revision=rev,
        torch_dtype=torch.bfloat16, device_map="cuda")
    sched = FlowMatchEulerDiscreteScheduler.from_pretrained(
        repo, subfolder="scheduler", revision=rev)
    tok = T5Tokenizer.from_pretrained(repo, subfolder="tokenizer", revision=rev)
    pipe = LTXConditionPipeline(scheduler=sched, vae=vae, text_encoder=None,
                                tokenizer=tok, transformer=transformer)
    try:
        pipe.vae.enable_tiling()
    except Exception:
        pass
    return {"pipe": pipe}


def generate(session: dict, profile: dict, mode_cfg: dict, image,
             seed: int, frames: int, fps: int) -> tuple[list, dict]:
    pipe = session["pipe"]
    embeds = {k: v.to("cuda") for k, v in session["embeds"].items()
              if isinstance(v, torch.Tensor)}
    meta = session["embeds"].get("_meta", {})
    out = pipe(
        image=image,
        frame_index=0,
        strength=1.0,
        height=profile["height"],
        width=profile["width"],
        num_frames=frames,
        frame_rate=fps,
        num_inference_steps=mode_cfg["steps"],
        guidance_scale=mode_cfg["guidance"],
        generator=cuda_generator(seed),
        output_type="pil",
        **embeds,
    ).frames[0]
    info = {
        "pipeline": "LTXConditionPipeline",
        "scheduler": type(pipe.scheduler).__name__,
        # negative embeds exist but are inert when do_classifier_free_guidance is off (CFG ~1.0)
        "negative_prompt_used": embeds.get("negative_prompt_embeds") is not None
                                and mode_cfg["guidance"] != 1.0,
        "conditioning": "first-frame (image, frame_index=0)",
        "prompt_tokens": meta.get("prompt_tokens"),
        "prompt_truncated_256tok": meta.get("prompt_truncated", False),
    }
    return out, info
