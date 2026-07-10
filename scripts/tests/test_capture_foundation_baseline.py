import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest

from scripts.capture_foundation_baseline import (
    BaselineCapture,
    BundleInventoryError,
    CommandResult,
    classify_bundle_asset,
    inventory_bundle,
    median,
    render_json,
    render_markdown,
    run_command,
    safe_environment,
    sha256_file,
)


class AggregationTests(unittest.TestCase):
    def test_median_handles_odd_and_even_sample_counts(self) -> None:
        self.assertEqual(median([9.0, 1.0, 5.0]), 5.0)
        self.assertEqual(median([8.0, 2.0, 4.0, 6.0]), 5.0)

    def test_median_rejects_an_empty_sample(self) -> None:
        with self.assertRaisesRegex(ValueError, "at least one sample"):
            median([])

    def test_sha256_file_is_deterministic(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            first = Path(directory) / "first.lock"
            second = Path(directory) / "second.lock"
            first.write_bytes(b"baseline\n")
            second.write_bytes(b"baseline\n")

            expected = hashlib.sha256(b"baseline\n").hexdigest()
            self.assertEqual(sha256_file(first), expected)
            self.assertEqual(sha256_file(second), expected)


class BundleInventoryTests(unittest.TestCase):
    def test_classifies_entry_chunks_and_other_assets(self) -> None:
        self.assertEqual(classify_bundle_asset("index.html"), "eager")
        self.assertEqual(classify_bundle_asset("app.js"), "eager")
        self.assertEqual(classify_bundle_asset("styles.css"), "eager")
        self.assertEqual(classify_bundle_asset("settings.js"), "lazy")
        self.assertEqual(classify_bundle_asset("routes/memory.css"), "lazy")
        self.assertEqual(classify_bundle_asset("noema-mark.svg"), "asset")

    def test_inventory_is_sorted_and_includes_sizes_and_hashes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "app.js").write_bytes(b"console.log('noema')\n")
            (root / "index.html").write_bytes(b"<main></main>\n")
            (root / "styles.css").write_bytes(b"main { color: black; }\n")
            (root / "routes").mkdir()
            (root / "routes" / "memory.js").write_bytes(b"export default 1\n")

            inventory = inventory_bundle(root)

            self.assertEqual(
                [entry["path"] for entry in inventory],
                ["app.js", "index.html", "routes/memory.js", "styles.css"],
            )
            self.assertTrue(all(entry["raw_bytes"] > 0 for entry in inventory))
            self.assertTrue(all(entry["gzip_bytes"] > 0 for entry in inventory))
            self.assertTrue(all(len(entry["sha256"]) == 64 for entry in inventory))

    def test_inventory_rejects_missing_required_entries(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "index.html").write_text("<main></main>", encoding="utf-8")
            (root / "app.js").write_text("export {};", encoding="utf-8")

            with self.assertRaisesRegex(
                BundleInventoryError, "missing required bundle entries: styles.css"
            ):
                inventory_bundle(root)


class CommandSafetyTests(unittest.TestCase):
    def test_optional_missing_command_is_recorded_as_unavailable(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            result = run_command(
                ["noema-definitely-missing-command", "--version"],
                cwd=Path(directory),
                optional=True,
            )

        self.assertFalse(result.available)
        self.assertEqual(result.record["exit_status"], 127)
        self.assertEqual(result.record["command"], ["noema-definitely-missing-command", "--version"])
        self.assertEqual(result.record["environment"], {})

    def test_safe_environment_keeps_only_build_relevant_keys(self) -> None:
        environment = {
            "CARGO_TARGET_DIR": "/tmp/noema-target",
            "CARGO_BUILD_RUSTC_WRAPPER": ".cargo/rustc-wrapper",
            "RUSTC_WRAPPER": "sccache",
            "CC": "sccache clang",
            "CXX": "sccache clang++",
            "PATH": "/bin",
            "OPENAI_API_KEY": "secret",
        }

        self.assertEqual(
            safe_environment(environment),
            {
                "CARGO_BUILD_RUSTC_WRAPPER": ".cargo/rustc-wrapper",
                "CARGO_TARGET_DIR": "/tmp/noema-target",
                "CC": "sccache clang",
                "CXX": "sccache clang++",
                "RUSTC_WRAPPER": "sccache",
            },
        )

    def test_sccache_stats_do_not_double_count_advanced_aliases(self) -> None:
        payload = {
            "stats": {
                "compile_requests": 13,
                "cache_errors": {
                    "counts": {"Rust": 1},
                    "adv_counts": {"rust": {"read_error": 1}},
                },
                "cache_hits": {
                    "counts": {"Rust": 4, "C/C++": 2},
                    "adv_counts": {"rust": {"disk": 4}, "c_cpp": {"disk": 2}},
                },
                "cache_misses": {
                    "counts": {"Rust": 2, "C/C++": 1},
                    "adv_counts": {"rust": {"disk": 2}, "c_cpp": {"disk": 1}},
                },
            },
            "cache_location": "/private/cache/path",
            "basedirs": ["/private/source/path"],
        }
        result = CommandResult(
            record={},
            stdout=json.dumps(payload),
            stderr="",
            available=True,
        )
        capture = BaselineCapture()
        capture.command = lambda *_args, **_kwargs: result  # type: ignore[method-assign]

        counters = capture.sccache_stats()

        self.assertEqual(counters["cache_errors"], 1)
        self.assertEqual(counters["cache_hits"], 6)
        self.assertEqual(counters["cache_misses"], 3)
        self.assertNotIn("cache_location", counters)


class RenderingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.baseline = {
            "schema_version": 1,
            "captured_at": "2026-07-09T12:00:00Z",
            "repository": {
                "commit": "abc123",
                "dirty": True,
                "dirty_summary": {"tracked": 2, "untracked": 1},
            },
            "environment": {
                "os": "TestOS 1",
                "architecture": "arm64",
                "cpu_model": "Test CPU",
                "tools": {
                    "bun": {"available": False, "version": None},
                    "rustc": {"available": True, "version": "rustc test"},
                },
            },
            "locks": {
                "Cargo.lock": {"present": True, "sha256": "a" * 64},
            },
            "cache": {
                "policy": {
                    "cargo_rustc_wrapper": ".cargo/rustc-wrapper",
                    "compiler_cache": "sccache",
                    "allowed_override": "CARGO_TARGET_DIR only",
                },
                "before": {"compile_requests": 10, "cache_hits": 7},
                "after": {"compile_requests": 12, "cache_hits": 9},
            },
            "frontend_bundle": [
                {
                    "path": "app.js",
                    "kind": "eager",
                    "raw_bytes": 100,
                    "gzip_bytes": 70,
                    "sha256": "b" * 64,
                }
            ],
            "rust_timings": {
                "clean-workspace-build": {
                    "command": ["cargo", "build", "--workspace", "--timings"],
                    "samples_seconds": [3.0, 2.0, 4.0],
                    "median_seconds": 3.0,
                }
            },
            "commands": [
                {
                    "command": ["cargo", "build", "--workspace", "--timings"],
                    "cwd": ".",
                    "environment": {"CARGO_TARGET_DIR": ".local/target"},
                    "exit_status": 0,
                    "elapsed_seconds": 3.0,
                    "stdout_sha256": "c" * 64,
                    "stderr_sha256": "d" * 64,
                    "output_artifacts": [],
                }
            ],
        }

    def test_json_rendering_is_deterministic(self) -> None:
        reordered = dict(reversed(list(self.baseline.items())))
        self.assertEqual(render_json(self.baseline), render_json(reordered))
        self.assertTrue(render_json(self.baseline).endswith("\n"))

    def test_markdown_rendering_is_deterministic_and_states_policy(self) -> None:
        first = render_markdown(self.baseline)
        second = render_markdown(self.baseline)

        self.assertEqual(first, second)
        self.assertIn("machine-specific and informational", first)
        self.assertIn("`.cargo/rustc-wrapper`", first)
        self.assertIn("`sccache`", first)
        self.assertIn("Only `CARGO_TARGET_DIR`", first)


if __name__ == "__main__":
    unittest.main()
