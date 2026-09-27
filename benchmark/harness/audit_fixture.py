import argparse
import base64
import hashlib
import importlib.metadata
import json
import pathlib
import struct
import sys
from datetime import datetime, timezone

import tiktoken


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def audit(root):
    manifest_path = root / "manifest/fixture.json"
    manifest = load(manifest_path)
    expected_paths = {
        "../assets/provisional-mascot.png",
        "decoder-vectors/vectors.json",
        "fixtures/chunks.json",
        "fixtures/history.json",
        "fixtures/response.txt",
        "fixtures/text.json",
        "harness/control-v1.json",
        "requirements-source.txt",
    }
    assert manifest["schema"] == "mascot-fixture-1"
    assert manifest["version"] == "windows-v1.0.1"
    assert set(manifest["files_sha256"]) == expected_paths
    for name, expected in manifest["files_sha256"].items():
        assert digest(root / name) == expected, name
    provider = manifest["provider"]
    assert digest(pathlib.Path(provider["path"])) == provider["sha256"]
    assert pathlib.Path(provider["path"]).resolve() == (root / "bin/fixture.exe").resolve()
    assert pathlib.Path(provider["cwd"]).resolve() == root
    assert provider["arguments"] == ["provider", str(root)]
    assert provider["inherit_environment"] is False
    assert provider["environment"] == {
        "SystemRoot": "C:\\Windows", "WINDIR": "C:\\Windows", "PATH": "C:\\Windows\\System32",
        "GOMAXPROCS": "1", "GOGC": "100", "GOMEMLIMIT": "off", "GODEBUG": "", "GOTRACEBACK": "none",
    }
    protocol = manifest["protocol"]
    for key, value in {
        "max_stdin_frame_bytes_including_lf": 65536,
        "max_stdout_frame_bytes_including_lf": 65536,
        "oversized_frame_bytes_including_lf": 65537,
        "normal_chunks": 100,
        "normal_interval_ms": 10,
        "shutdown_timeout_ms": 2000,
        "cancel_timeout_ms": 1000,
        "unexpected_exit_code": 23,
    }.items():
        assert protocol[key] == value, key
    text = load(root / "fixtures/text.json")
    assert text["F1"] == "Ae\u0301B"
    assert text["F2"] == "A\U0001f468\u200d\U0001f4bbB"
    assert text["F3"] == "English العربية English"
    assert text["F10"] == "first line\nĀrā līst\nПроверка\n\U0001f468\u200d\U0001f4bb"
    for name in ("F1", "F2"):
        selection = text[name + "_selection"]
        units = text[name].encode("utf-16-le")
        start, end = selection["utf16_selection_start"] * 2, selection["utf16_selection_end"] * 2
        assert units[start:end].decode("utf-16-le") == selection["copied"]
        assert (units[:start] + units[end:]).decode("utf-16-le") == "AB"
    assert text["permitted_native_variations"] == []
    assert text["F6"]["ime"] == "Microsoft Japanese IME"
    assert text["F6"]["romaji"] == "nihonn"
    assert text["F6"]["expected"] == "にほん"
    chunks = load(root / "fixtures/chunks.json")
    assert len(chunks) == 100
    response = "".join(chunks)
    assert response.encode("utf-8") == (root / "fixtures/response.txt").read_bytes()
    for fragment in [text["F1"], text["F2"], text["F3"], *text["F4"][:2]]:
        assert fragment in response
    for emoji_sequence in text["F4"][2].split():
        assert emoji_sequence in response
    assert len(load(root / "fixtures/history.json")) == 4
    vector_checks = []
    for vector in load(root / "decoder-vectors/vectors.json"):
        pending = bytearray()
        frames = []
        rejected = False
        peak = 0
        for fragment in vector["fragments_base64"]:
            for byte in base64.b64decode(fragment, validate=True):
                if len(pending) == 65536:
                    rejected = True
                    break
                pending.append(byte)
                peak = max(peak, len(pending))
                if byte == 10:
                    json.loads(pending.decode("utf-8"))
                    frames.append(bytes(pending))
                    pending.clear()
            if rejected:
                break
        assert rejected == vector["reject"], vector["name"]
        expected = [base64.b64decode(frame, validate=True) for frame in (vector["expected_frames_base64"] or [])]
        assert frames == expected, vector["name"]
        assert rejected or not pending, vector["name"]
        vector_checks.append({"name": vector["name"], "peak_buffer_bytes": peak, "status": "PASS"})
    png = (root / "../assets/provisional-mascot.png").read_bytes()
    assert png[:8] == b"\x89PNG\r\n\x1a\n" and png[12:16] == b"IHDR"
    width, height, depth, color = struct.unpack(">IIBB", png[16:26])
    assert (width, height, depth, color) == (128, 128, 8, 6)
    assert manifest["asset"]["status"] == "PROVISIONAL"
    assert manifest["asset"]["logical_width_dip"] == manifest["asset"]["logical_height_dip"] == 64
    schedule = manifest["schedule"]
    assert sorted(tuple(block) for block in schedule["balanced_blocks"]) == sorted([
        ("rust", "zig", "go"), ("zig", "go", "rust"), ("go", "rust", "zig"),
        ("go", "zig", "rust"), ("zig", "rust", "go"), ("rust", "go", "zig"),
    ])
    for key, value in {
        "fresh_launches_each": 30, "first_activation_lifetimes_each": 10,
        "post_reboot_first_launches_each": 3, "steady_state_independent_runs": 3,
        "steady_settle_ms": 60000, "steady_collection_ms": 30000,
        "active_sample_ms": 100, "focused_idle_ms": 600000,
        "stability_batches": 3, "operations_per_batch": 100, "stability_sample_every": 10,
    }.items():
        assert schedule[key] == value, key
    assert schedule["warm_activations_per_lifetime"] == [34, 33, 33]
    assert manifest["observer"]["qualification_required_before_headline"] is True
    packages = {}
    for requirement in (root / "requirements-source.txt").read_text(encoding="utf-8").splitlines():
        name, expected = requirement.split("==")
        packages[name] = importlib.metadata.version(name)
        assert packages[name] == expected, name
    assert packages["tiktoken"] == manifest["source_efficiency"]["package_version"]
    assert manifest["source_efficiency"]["tokenizer"] == "tiktoken/o200k_base"
    encoding = tiktoken.get_encoding("o200k_base")
    assert encoding.decode(encoding.encode(response, disallowed_special=())) == response
    ime = load(root / "results/windows/preflight/ime-009/observations.json")
    review = load(root / "results/windows/preflight/ime-009/review.json")
    for key in ("Japanese IME available", "preedit/composition visible", "commit works", "cancel works"):
        assert review[key] == "YES"
    assert ime["preedit"] == ime["committed_text"] == "にほん"
    assert ime["text_after_cancel"] == ime["prior_text_for_cancel"]
    assert ime["composing_before_commit"] and ime["composing_before_cancel"]
    assert not ime["composing_after_commit"] and not ime["composing_after_cancel"]
    return {
        "schema": "mascot-independent-fixture-audit-1",
        "utc": datetime.now(timezone.utc).isoformat(),
        "fixture_version": manifest["version"],
        "manifest_sha256": digest(manifest_path),
        "provider_sha256": provider["sha256"],
        "payload_sha256": digest(root / "fixtures/response.txt"),
        "asset_sha256": digest(root / "../assets/provisional-mascot.png"),
        "packages": packages,
        "python": sys.version,
        "vectors": vector_checks,
        "status": "PASS",
        "scope": "Independent fixture identity/schema/payload/vector/schedule and installed tokenizer checks; verifies the recorded IME review, does not replace visual review or candidate correctness.",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=pathlib.Path)
    parser.add_argument("output", type=pathlib.Path)
    args = parser.parse_args()
    result = audit(args.root.resolve())
    with args.output.open("x", encoding="utf-8", newline="\n") as stream:
        json.dump(result, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    print(args.output)


if __name__ == "__main__":
    main()
