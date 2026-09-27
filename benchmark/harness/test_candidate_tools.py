import copy
import ctypes
import json
import os
import pathlib
import subprocess
import sys
import unittest

import verify_candidate_smoke as smoke
import verify_provider_regression as regression


class ContractHelpers(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        root = pathlib.Path(__file__).resolve().parent.parent
        cls.chunks = smoke.load(root / "fixtures/chunks.json")
        cls.vectors = smoke.load(root / "decoder-vectors/vectors.json")
        cls.response = (root / "fixtures/response.txt").read_text(encoding="utf-8")

    def test_reconstruction_cases(self):
        expected_cases = {
            "normal": (self.response, 100),
            "fragmented": (self.response, 100),
            "client_request": (self.response, 100),
            "stderr": (self.response, 100),
            "cancel": ("".join(self.chunks[:50]), 50),
            "unexpected_exit": ("".join(self.chunks[:7]), 7),
            "oversized": ("", 0),
            "backpressure": ("".join(self.chunks[index % 100] for index in range(256)), 256),
        }
        for name, expected in expected_cases.items():
            with self.subTest(name=name):
                self.assertEqual(regression.expected_text(name, 7, self.chunks, self.vectors, 65536), expected)

    def test_maximum_frame_tracks_id_width(self):
        for request_id in (1, 9, 10, 99999, 9223372036854775807):
            with self.subTest(request_id=request_id):
                text, count = regression.expected_text("maximum", request_id, self.chunks, self.vectors, 65536)
                wire = (json.dumps(dict(type="chunk", id=request_id, seq=0, text=text, emit_qpc="0" * 20),
                                   sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")
                self.assertEqual(len(wire), 65536)
                self.assertEqual(count, 1)

    def test_vector_buffer_bounds(self):
        for vector in self.vectors:
            with self.subTest(name=vector["name"]):
                peak = smoke.vector_peak(vector, 65536)
                self.assertLessEqual(peak, 65536)
                if vector["name"] in ("maximum_valid", "oversized"):
                    self.assertEqual(peak, 65536)

    @staticmethod
    def events():
        return [dict(event="frame_received", request_id=7, seq=0, receipt_qpc="101",
                     emit_qpc="00000000000000000100", qpc_frequency=10000000),
                dict(event="chunk_accepted", request_id=7, seq=0, accepted_qpc="102"),
                dict(event="terminal", request_id=7, kind="complete", last_seq=0, qpc="103")]

    def test_event_contract_accepts_valid_trace(self):
        self.assertEqual(regression.verify_events(self.events(), 7, 1, "complete", 10000000)["qpc"], "103")

    def test_active_shutdown_stream_does_not_require_terminal(self):
        rows = self.events()[:2]
        self.assertEqual(regression.verify_event_stream(rows, 7, 1, 10000000), [])
        with self.assertRaises(AssertionError):
            regression.verify_events(rows, 7, 1, "complete", 10000000)

    def test_event_contract_rejects_faults(self):
        changes = [lambda rows: rows.append(copy.deepcopy(rows[-1])),
                   lambda rows: rows[0].update(seq=1),
                   lambda rows: rows[1].update(seq=False),
                   lambda rows: rows[0].update(qpc_frequency=10000000.0),
                   lambda rows: rows[0].update(emit_qpc="100"),
                   lambda rows: rows[0].update(receipt_qpc="99"),
                   lambda rows: rows[1].update(accepted_qpc="100"),
                   lambda rows: rows[2].update(last_seq=1),
                   lambda rows: rows[2].update(qpc="101"),
                   lambda rows: rows[2].update(request_id=8)]
        for change in changes:
            rows = self.events()
            change(rows)
            with self.subTest(rows=rows), self.assertRaises(AssertionError):
                regression.verify_events(rows, 7, 1, "complete", 10000000)

    def test_decimal_contract(self):
        self.assertEqual(regression.decimal("123"), 123)
        for value in (0, "0", "-1", "1.0", "١", str(2 ** 63)):
            with self.subTest(value=value), self.assertRaises(AssertionError):
                regression.decimal(value)


@unittest.skipUnless(os.name == "nt", "Native Windows API qualification")
class NativeHelpers(unittest.TestCase):
    def test_struct_layout(self):
        self.assertEqual(ctypes.sizeof(smoke.LogFont), 92)
        self.assertEqual(ctypes.sizeof(regression.ProcessEntry), 568 if ctypes.sizeof(ctypes.c_void_p) == 8 else 556)

    def test_owned_process_exit_observation(self):
        native = regression.Native()
        process = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(0.25); raise SystemExit(17)"],
                                   creationflags=subprocess.CREATE_NO_WINDOW)
        child = None
        watch = None
        try:
            child = regression.Child(native, process.pid, os.getpid(), pathlib.Path(sys.executable).name)
            self.assertTrue(child.alive())
            watch = regression.ExitWatch(child, native.counter(), 2000, 17)
            row = watch.finish()
            self.assertEqual(row["exit_code"], 17)
            self.assertEqual(row["pid"], process.pid)
            self.assertGreater(child.creation, 0)
            self.assertFalse(child.alive())
            self.assertEqual(process.wait(timeout=2), 17)
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=2)
            if watch is not None:
                watch.thread.join(timeout=3)
            if child is not None:
                child.close()


if __name__ == "__main__":
    unittest.main()
