import torch
from diffusers import (AutoencoderKLWan, UniPCMultistepScheduler,
                       WanImageToVideoPipeline, WanTransformer3DModel)
from diffusers.quantizers.quantization_config import BitsAndBytesConfig
from transformers import T5TokenizerFast

from .common import (cast_embeds, cuda_generator, encode_in_subprocess,
                     load_int8_text_encoder)

BACKEND = "wan_ti2v"


def encode_prompt_embeds(repo: str, revision, prompt: str,
                         negative_prompt: str | None) -> dict:
    tok = T5TokenizerFast.from_pretrained(repo, subfolder="tokenizer", revision=revision)
    te = load_int8_text_encoder(repo, "UMT5EncoderModel", revision)
    enc_pipe = WanImageToVideoPipeline(
        tokenizer=tok, text_encoder=te, vae=None, scheduler=None,
        transformer=None, transformer_2=None, image_encoder=None,
        image_processor=None, boundary_ratio=0.0)
    res = enc_pipe.encode_prompt(
        prompt=prompt, negative_prompt=negative_prompt,
        do_classifier_free_guidance=True, device="cuda")
    embeds = cast_embeds(
        dict(zip(("prompt_embeds", "negative_prompt_embeds"), res)),
        torch.bfloat16)
    embeds["_meta"] = {
        "prompt_tokens": len(tok(prompt, truncation=False).input_ids),
    }
    return embeds


def encode(profile: dict, prompt: str, negative_prompt: str | None,
           out_pt) -> float:
    return encode_in_subprocess(BACKEND, profile["hf_repo"],
                                profile.get("hf_revision") or None,
                                prompt, negative_prompt, out_pt)


def load_pipe(profile: dict) -> dict:
    repo = profile["hf_repo"]
    rev = profile.get("hf_revision") or None
    # 5B transformer bf16 (~9.3 GB) + fp32 VAE (2.6 GB) would exceed the
    # ~10 GB free VRAM budget once desktop overhead is counted — int8 keeps
    # the transformer at ~5.3 GB (quantization noted in the receipt).
    transformer = WanTransformer3DModel.from_pretrained(
        repo, subfolder="transformer", revision=rev,
        quantization_config=BitsAndBytesConfig(load_in_8bit=True),
        device_map="cuda")
    vae = AutoencoderKLWan.from_pretrained(
        repo, subfolder="vae", revision=rev,
        torch_dtype=torch.float32, device_map="cuda")
    sched = UniPCMultistepScheduler.from_pretrained(
        repo, subfolder="scheduler", revision=rev)
    tok = T5TokenizerFast.from_pretrained(repo, subfolder="tokenizer", revision=rev)
    # TI2V-5B: single-stage (boundary_ratio=0.0) and expand_timesteps=True —
    # conditioning blends image latent into frame 0 instead of channel-concat
    pipe = WanImageToVideoPipeline(
        tokenizer=tok, text_encoder=None, vae=vae, scheduler=sched,
        transformer=transformer, transformer_2=None, image_encoder=None,
        image_processor=None, boundary_ratio=0.0, expand_timesteps=True)
    return {"pipe": pipe}


def generate(session: dict, profile: dict, mode_cfg: dict, image,
             seed: int, frames: int, fps: int) -> tuple[list, dict]:
    pipe = session["pipe"]
    embeds = {k: v.to("cuda") for k, v in session["embeds"].items()
              if isinstance(v, torch.Tensor)}
    meta = session["embeds"].get("_meta", {})
    out = pipe(
        image=image,
        height=profile["height"],
        width=profile["width"],
        num_frames=frames,
        num_inference_steps=mode_cfg["steps"],
        guidance_scale=mode_cfg["guidance"],
        generator=cuda_generator(seed),
        output_type="pil",
        **embeds,
    ).frames[0]
    info = {
        "pipeline": "WanImageToVideoPipeline",
        "scheduler": type(pipe.scheduler).__name__,
        "negative_prompt_used": embeds.get("negative_prompt_embeds") is not None,
        "conditioning": "first-frame via VAE latent (no CLIP image encoder; TI2V-5B single stage)",
        "prompt_tokens": meta.get("prompt_tokens"),
    }
    return out, info
