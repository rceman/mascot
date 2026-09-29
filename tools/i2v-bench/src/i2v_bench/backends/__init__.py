from importlib import import_module

_BACKENDS = {
    "ltx": ".ltx",
    "wan_vace": ".wan_vace",
    "wan_ti2v": ".wan_ti2v",
}


def get_backend(kind: str):
    if kind not in _BACKENDS:
        raise KeyError(f"unknown backend '{kind}'; available: {sorted(_BACKENDS)}")
    return import_module(_BACKENDS[kind], __package__)
