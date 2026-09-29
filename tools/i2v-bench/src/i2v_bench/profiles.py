import json

from .config import PROFILES_PATH


def load_profiles() -> dict:
    return json.loads(PROFILES_PATH.read_text(encoding="utf-8"))["profiles"]


def get_profile(name: str) -> dict:
    profiles = load_profiles()
    if name not in profiles:
        raise KeyError(f"unknown profile '{name}'; available: {sorted(profiles)}")
    p = dict(profiles[name])
    p["name"] = name
    return p
