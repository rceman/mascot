import os
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
PROFILES_PATH = REPO_ROOT / "benchmark" / "i2v" / "configs" / "profiles.json"
DEFAULT_RESULTS = REPO_ROOT / "benchmark" / "i2v" / "results" / "windows" / "local-i2v-v0.1"
DERIVED_ASSETS = REPO_ROOT / "assets" / "i2v"


def model_cache() -> Path:
    return Path(os.environ.get("MASCOT_I2V_MODEL_CACHE", r"W:\devin_folder\mascot-i2v-model-cache"))


def setup_hf_env() -> Path:
    """Point HF caches at the external model cache. Must run before importing HF libs."""
    cache = model_cache()
    os.environ.setdefault("HF_HOME", str(cache / "hf-home"))
    os.environ.setdefault("HF_HUB_CACHE", str(cache / "hf-home" / "hub"))
    os.environ.setdefault("HF_HUB_DISABLE_SYMLINKS_WARNING", "1")
    return cache
