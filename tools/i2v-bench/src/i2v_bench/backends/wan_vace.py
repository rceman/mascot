import torch
from PIL import Image
from diffusers import (AutoencoderKLWan, UniPCMultistepScheduler,
                       WanVACEPipeline, WanVACETransformer3DModel)
from transformers import T5TokenizerFast

from .common import (cast_embeds, cuda_generator, encode_in_subprocess,
                     load_int8_text_encoder)

BACKEND = "wan_vace"


def _first_frame_conditioning(image, width: int, height: int, num_frames: int):
    """VACE I2V: keep frame 0 (mask=0/black), generate the rest (mask=255/white)."""
    first = image.resize((width, height))
    gray = Image.new("RGB", (width, height), (128, 128, 128))
    video = [first, *([gray] * (num_frames - 1))]
    mask = [Image.new("L", (width, height), 0),
            *([Image.new("L", (width, height), 255)] * (num_frames - 1))]
    return video, mask


def encode_prompt_embeds(repo: str, revision, prompt: str,
                         negative_prompt: str | None) -> dict:
    tok = T5TokenizerFast.from_pretrained(repo, subfolder="tokenizer", revision=revision)
    te = load_int8_text_encoder(repo, "UMT5EncoderModel", revision)
    enc_pipe = WanVACEPipeline(tokenizer=tok, text_encoder=te, vae=None,
                               scheduler=None, transformer=None,
                               transformer_2=None)
    res = enc_pipe.encode_prompt(prompt=prompt, negative_prompt=negative_prompt,
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
    transformer = WanVACETransformer3DModel.from_pretrained(
        repo, subfolder="transformer", revision=rev,
        torch_dtype=torch.bfloat16, device_map="cuda")
    vae = AutoencoderKLWan.from_pretrained(
        repo, subfolder="vae", revision=rev,
        torch_dtype=torch.float32, device_map="cuda")
    flow_shift = profile.get("flow_shift", 3.0)
    sched = UniPCMultistepScheduler.from_pretrained(repo, subfolder="scheduler", revision=rev)
    sched = UniPCMultistepScheduler.from_config(sched.config, flow_shift=flow_shift)
    tok = T5TokenizerFast.from_pretrained(repo, subfolder="tokenizer", revision=rev)
    pipe = WanVACEPipeline(tokenizer=tok, text_encoder=None, vae=vae,
                           scheduler=sched, transformer=transformer,
                           transformer_2=None)
    return {"pipe": pipe}


def generate(session: dict, profile: dict, mode_cfg: dict, image,
             seed: int, frames: int, fps: int) -> tuple[list, dict]:
    pipe = session["pipe"]
    embeds = {k: v.to("cuda") for k, v in session["embeds"].items()
              if isinstance(v, torch.Tensor)}
    meta = session["embeds"].get("_meta", {})
    video, mask = _first_frame_conditioning(
        image, profile["width"], profile["height"], frames)
    out = pipe(
        video=video,
        mask=mask,
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
        "pipeline": "WanVACEPipeline",
        "scheduler": f"UniPCMultistepScheduler(flow_shift={profile.get('flow_shift', 3.0)})",
        "negative_prompt_used": embeds.get("negative_prompt_embeds") is not None,
        "conditioning": "first-frame via VACE video+mask",
        "prompt_tokens": meta.get("prompt_tokens"),
    }
    return out, info
