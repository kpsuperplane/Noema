#!/usr/bin/env python3
"""Capture the machine-specific Foundation build and bundle baseline."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from datetime import datetime, timezone
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import subprocess
import sys
import time
from typing import Any, Iterable, Mapping, Optional, Sequence
import uuid


ROOT = Path(__file__).resolve().parents[1]
OUTPUT_DIR = ROOT / "docs" / "engineering" / "baselines"
JSON_PATH = OUTPUT_DIR / "foundation.json"
MARKDOWN_PATH = OUTPUT_DIR / "foundation.md"
BUNDLE_DIR = ROOT / "crates" / "noema-core" / "target" / "web-assets"
WEB_DIR = ROOT / "crates" / "noema-core" / "web"
MEASUREMENT_ROOT = ROOT / ".local" / "remediation-baselines" / "foundation"
TARGET_ROOT = MEASUREMENT_ROOT / "targets"

REQUIRED_BUNDLE_ENTRIES = ("index.html", "app.js", "styles.css")
SAFE_ENVIRONMENT_KEYS = (
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_TARGET_DIR",
    "CC",
    "CXX",
    "RUSTC_WRAPPER",
)
FORBIDDEN_OVERRIDES = {
    "CARGO_BUILD_RUSTC_WRAPPER",
    "RUSTC_WRAPPER",
    "CC",
    "CXX",
}
TOOL_COMMANDS = {
    "rustc": ["rustc", "-Vv"],
    "cargo": ["cargo", "-V"],
    "bun": ["bun", "-v"],
    "python": ["python3", "--version"],
    "swift": ["swift", "--version"],
    "xcode": ["xcodebuild", "-version"],
    "tauri_cli": ["cargo", "tauri", "--version"],
}
SCCACHE_COUNTER_KEYS = (
    "compile_requests",
    "requests_unsupported_compiler",
    "requests_not_compile",
    "requests_not_cacheable",
    "requests_executed",
    "cache_errors",
    "cache_hits",
    "cache_misses",
    "cache_timeouts",
    "cache_read_errors",
    "non_cacheable_compilations",
    "forced_recaches",
    "cache_write_errors",
    "cache_writes",
    "compilations",
    "compile_fails",
    "dist_errors",
)
SCCACHE_CANONICAL_MAP_COUNTERS = {"cache_errors", "cache_hits", "cache_misses"}


class BaselineError(RuntimeError):
    """Raised when a required baseline operation cannot complete safely."""


class BundleInventoryError(BaselineError):
    """Raised when generated frontend bundle entries are incomplete."""


@dataclass(frozen=True)
class CommandResult:
    """Captured command result plus transient output needed for aggregation."""

    record: dict[str, Any]
    stdout: str
    stderr: str
    available: bool


def median(values: Sequence[float]) -> float:
    """Return the median of a non-empty numeric sample."""

    if not values:
        raise ValueError("median requires at least one sample")
    ordered = sorted(float(value) for value in values)
    middle = len(ordered) // 2
    if len(ordered) % 2:
        return ordered[middle]
    return (ordered[middle - 1] + ordered[middle]) / 2


def sha256_bytes(content: bytes) -> str:
    """Return a SHA-256 digest for bytes."""

    return hashlib.sha256(content).hexdigest()


def sha256_file(path: Path) -> str:
    """Return a streaming SHA-256 digest for a file."""

    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def classify_bundle_asset(relative_path: str) -> str:
    """Classify generated assets by their initial-load behavior."""

    normalized = relative_path.replace(os.sep, "/")
    if normalized in REQUIRED_BUNDLE_ENTRIES:
        return "eager"
    suffix = Path(normalized).suffix.lower()
    if suffix in {".js", ".css"}:
        return "lazy"
    return "asset"


def inventory_bundle(bundle_dir: Path) -> list[dict[str, Any]]:
    """Inventory a complete generated frontend bundle deterministically."""

    missing = [name for name in REQUIRED_BUNDLE_ENTRIES if not (bundle_dir / name).is_file()]
    if missing:
        raise BundleInventoryError(
            "missing required bundle entries: " + ", ".join(sorted(missing))
        )

    inventory = []
    for path in sorted(item for item in bundle_dir.rglob("*") if item.is_file()):
        content = path.read_bytes()
        relative_path = path.relative_to(bundle_dir).as_posix()
        inventory.append(
            {
                "path": relative_path,
                "kind": classify_bundle_asset(relative_path),
                "raw_bytes": len(content),
                "gzip_bytes": len(gzip.compress(content, mtime=0)),
                "sha256": sha256_bytes(content),
            }
        )
    return inventory


def safe_environment(environment: Mapping[str, str]) -> dict[str, str]:
    """Return only non-secret environment values relevant to build reproduction."""

    return {
        key: environment[key]
        for key in SAFE_ENVIRONMENT_KEYS
        if key in environment
    }


def sanitize_sccache_stats(payload: Mapping[str, Any]) -> dict[str, int]:
    """Select aggregate counters from sccache's canonical current-schema fields."""

    stats = payload.get("stats")
    if not isinstance(stats, Mapping):
        raise BaselineError("sccache stats payload is missing the stats object")

    def sum_counter_map(counter_map: Any) -> int:
        if isinstance(counter_map, bool):
            return 0
        if isinstance(counter_map, (int, float)):
            return int(counter_map)
        if isinstance(counter_map, Mapping):
            return sum(sum_counter_map(value) for value in counter_map.values())
        return 0

    counters = {}
    for key in SCCACHE_COUNTER_KEYS:
        value = stats.get(key, 0)
        if key in SCCACHE_CANONICAL_MAP_COUNTERS:
            value = value.get("counts", {}) if isinstance(value, Mapping) else {}
        counters[key] = sum_counter_map(value)
    return counters


def _relative_display(path: Path) -> str:
    try:
        return path.resolve().relative_to(ROOT).as_posix() or "."
    except ValueError:
        return str(path.resolve())


def _artifact_record(path: Path) -> dict[str, Any]:
    record: dict[str, Any] = {
        "path": _relative_display(path),
        "present": path.is_file(),
    }
    if path.is_file():
        record.update({"bytes": path.stat().st_size, "sha256": sha256_file(path)})
    return record


def run_command(
    command: Sequence[str],
    *,
    cwd: Path,
    optional: bool = False,
    extra_env: Optional[Mapping[str, str]] = None,
    output_artifacts: Iterable[Path] = (),
) -> CommandResult:
    """Run and record one command without persisting the full environment."""

    overrides = dict(extra_env or {})
    forbidden = sorted(FORBIDDEN_OVERRIDES.intersection(overrides))
    if forbidden:
        raise BaselineError(
            "refusing to override protected build environment: " + ", ".join(forbidden)
        )
    unsupported = sorted(set(overrides).difference({"CARGO_TARGET_DIR"}))
    if unsupported:
        raise BaselineError(
            "only CARGO_TARGET_DIR may be set for measurements; got "
            + ", ".join(unsupported)
        )

    effective_environment = dict(os.environ)
    effective_environment.update(overrides)
    started = time.perf_counter()
    try:
        completed = subprocess.run(
            list(command),
            cwd=cwd,
            env=effective_environment if overrides else None,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        exit_status = completed.returncode
        stdout_bytes = completed.stdout
        stderr_bytes = completed.stderr
        command_found = True
    except FileNotFoundError as error:
        exit_status = 127
        stdout_bytes = b""
        stderr_bytes = str(error).encode("utf-8", errors="replace")
        command_found = False
    elapsed = time.perf_counter() - started

    record = {
        "command": list(command),
        "cwd": _relative_display(cwd),
        "environment": safe_environment(effective_environment),
        "exit_status": exit_status,
        "elapsed_seconds": round(elapsed, 6),
        "stdout_sha256": sha256_bytes(stdout_bytes),
        "stderr_sha256": sha256_bytes(stderr_bytes),
        "output_artifacts": [_artifact_record(path) for path in output_artifacts],
    }
    result = CommandResult(
        record=record,
        stdout=stdout_bytes.decode("utf-8", errors="replace"),
        stderr=stderr_bytes.decode("utf-8", errors="replace"),
        available=command_found and (exit_status == 0 or not optional),
    )
    if not optional and exit_status != 0:
        detail = result.stderr.strip() or result.stdout.strip() or "no command output"
        raise BaselineError(
            f"command failed with exit {exit_status}: {shlex.join(command)}\n{detail[-2000:]}"
        )
    return result


def render_json(baseline: Mapping[str, Any]) -> str:
    """Render stable machine-readable baseline JSON."""

    return json.dumps(baseline, indent=2, sort_keys=True, ensure_ascii=False) + "\n"


def _markdown_cell(value: Any) -> str:
    return str(value).replace("|", "\\|").replace("\n", "<br>")


def render_markdown(baseline: Mapping[str, Any]) -> str:
    """Render a concise deterministic summary of captured baseline data."""

    repository = baseline["repository"]
    environment = baseline["environment"]
    cache = baseline["cache"]
    lines = [
        "# Noema Foundation Reproducibility Baseline",
        "",
        f"Captured at `{baseline['captured_at']}` from commit `{repository['commit']}`.",
        "",
        "> Timings are machine-specific and informational. Foundation does not enforce performance thresholds.",
        "",
        "## Cache and wrapper policy",
        "",
        "Cargo must use the repository-configured `.cargo/rustc-wrapper`, which delegates every Rust compilation to `sccache`. Never run `cargo clean`, clear/reset cache state, or set, unset, or modify `CARGO_BUILD_RUSTC_WRAPPER`, `RUSTC_WRAPPER`, `CC`, or `CXX`. Only `CARGO_TARGET_DIR` may be set, solely for isolated measurement targets.",
        "",
        f"Wrapper SHA-256: `{cache['policy'].get('wrapper_sha256', 'unavailable')}`",
        "",
        "## Machine and tools",
        "",
        f"- OS: `{environment['os']}`",
        f"- Architecture: `{environment['architecture']}`",
        f"- CPU: `{environment['cpu_model']}`",
        f"- Repository dirty: `{str(repository['dirty']).lower()}` ({_markdown_cell(repository['dirty_summary'])})",
        "",
        "| Tool | Status | Version |",
        "| --- | --- | --- |",
    ]
    for name, tool in sorted(environment["tools"].items()):
        status = "available" if tool["available"] else "unavailable"
        version = tool["version"] if tool["version"] is not None else "—"
        lines.append(f"| `{name}` | {status} | {_markdown_cell(version)} |")

    lines.extend(["", "## Dependency locks", "", "| Path | Present | SHA-256 |", "| --- | --- | --- |"])
    for path, lock in sorted(baseline["locks"].items()):
        digest = f"`{lock['sha256']}`" if lock.get("sha256") else "—"
        lines.append(f"| `{path}` | {str(lock['present']).lower()} | {digest} |")

    lines.extend(["", "## Rust timings", "", "| Scenario | Command | Samples (seconds) | Median (seconds) |", "| --- | --- | --- | ---: |"])
    for name, timing in sorted(baseline["rust_timings"].items()):
        samples = ", ".join(f"{sample:.6f}" for sample in timing["samples_seconds"])
        command = shlex.join(timing["command"])
        lines.append(
            f"| `{name}` | `{command}` | {samples} | {timing['median_seconds']:.6f} |"
        )

    lines.extend(["", "## Frontend bundle", "", "| Path | Kind | Raw bytes | Gzip bytes | SHA-256 |", "| --- | --- | ---: | ---: | --- |"])
    for asset in sorted(baseline["frontend_bundle"], key=lambda item: item["path"]):
        lines.append(
            f"| `{asset['path']}` | {asset['kind']} | {asset['raw_bytes']} | {asset['gzip_bytes']} | `{asset['sha256']}` |"
        )

    lines.extend(["", "## Command evidence", "", "Every command records its exit status, elapsed time, allowlisted relevant environment, and stdout/stderr or artifact hashes in `foundation.json`.", "", "| # | Command | Exit | Seconds | Relevant environment |", "| ---: | --- | ---: | ---: | --- |"])
    for index, command in enumerate(baseline["commands"], 1):
        rendered_environment = ", ".join(
            f"{key}={value}" for key, value in sorted(command["environment"].items())
        ) or "—"
        lines.append(
            f"| {index} | `{_markdown_cell(shlex.join(command['command']))}` | {command['exit_status']} | {command['elapsed_seconds']:.6f} | `{_markdown_cell(rendered_environment)}` |"
        )
    return "\n".join(lines) + "\n"


class BaselineCapture:
    """Sequentially capture metadata, builds, checks, and cache counters."""

    def __init__(self) -> None:
        self.commands: list[dict[str, Any]] = []

    def command(self, command: Sequence[str], **kwargs: Any) -> CommandResult:
        try:
            result = run_command(command, **kwargs)
        except BaselineError:
            raise
        self.commands.append(result.record)
        return result

    def optional_command(self, command: Sequence[str], *, cwd: Path) -> CommandResult:
        result = run_command(command, cwd=cwd, optional=True)
        self.commands.append(result.record)
        return result

    def tool_versions(self) -> dict[str, dict[str, Any]]:
        tools = {}
        for name, command in TOOL_COMMANDS.items():
            result = self.optional_command(command, cwd=ROOT)
            available = result.record["exit_status"] == 0
            output = "\n".join(
                part.strip() for part in (result.stdout, result.stderr) if part.strip()
            )
            tools[name] = {
                "available": available,
                "version": output if available else None,
            }
        return tools

    def cpu_model(self) -> str:
        if platform.system() == "Darwin":
            result = self.optional_command(
                ["sysctl", "-n", "machdep.cpu.brand_string"], cwd=ROOT
            )
            if result.record["exit_status"] == 0 and result.stdout.strip():
                return result.stdout.strip()
        elif platform.system() == "Linux":
            cpuinfo = Path("/proc/cpuinfo")
            if cpuinfo.is_file():
                for line in cpuinfo.read_text(encoding="utf-8", errors="replace").splitlines():
                    if line.lower().startswith(("model name", "hardware")) and ":" in line:
                        return line.split(":", 1)[1].strip()
        return platform.processor() or platform.machine() or "unavailable"

    def repository_metadata(self) -> dict[str, Any]:
        commit = self.command(["git", "rev-parse", "HEAD"], cwd=ROOT).stdout.strip()
        status = self.command(
            ["git", "status", "--short", "--untracked-files=all"], cwd=ROOT
        ).stdout
        summary = {"tracked": 0, "untracked": 0, "staged": 0, "unstaged": 0}
        for line in status.splitlines():
            if line.startswith("??"):
                summary["untracked"] += 1
                continue
            summary["tracked"] += 1
            if len(line) >= 2 and line[0] != " ":
                summary["staged"] += 1
            if len(line) >= 2 and line[1] != " ":
                summary["unstaged"] += 1
        return {"commit": commit, "dirty": bool(status.strip()), "dirty_summary": summary}

    def sccache_stats(self) -> dict[str, int]:
        result = self.command(
            ["sccache", "--show-stats", "--stats-format", "json"], cwd=ROOT
        )
        try:
            payload = json.loads(result.stdout)
        except json.JSONDecodeError as error:
            raise BaselineError(f"could not parse sccache aggregate stats: {error}") from error
        return sanitize_sccache_stats(payload)

    def cargo_measurement(
        self, command: Sequence[str], target: Path
    ) -> CommandResult:
        target.mkdir(parents=True, exist_ok=False)
        return self.cargo_in_existing_target(command, target)

    def cargo_in_existing_target(
        self, command: Sequence[str], target: Path
    ) -> CommandResult:
        target_relative = target.relative_to(ROOT).as_posix()
        timing_path = target / "cargo-timings" / "cargo-timing.html"
        result = self.command(
            command,
            cwd=ROOT,
            extra_env={"CARGO_TARGET_DIR": target_relative},
            output_artifacts=[timing_path],
        )
        if not timing_path.is_file():
            raise BaselineError(
                f"Cargo did not produce required timing artifact: {timing_path}"
            )
        return result

    def timing_samples(
        self,
        command: Sequence[str],
        targets: Sequence[Path],
    ) -> dict[str, Any]:
        samples = [
            self.cargo_measurement(command, target).record["elapsed_seconds"]
            for target in targets
        ]
        return {
            "command": list(command),
            "samples_seconds": samples,
            "median_seconds": round(median(samples), 6),
        }

    def incremental_samples(
        self,
        name: str,
        trigger_path: Path,
        target: Path,
    ) -> dict[str, Any]:
        command = ["cargo", "check", "--workspace", "--timings"]
        samples = []
        for _ in range(3):
            original = trigger_path.stat()
            try:
                os.utime(
                    trigger_path,
                    ns=(original.st_atime_ns, time.time_ns()),
                )
                result = self.cargo_in_existing_target(command, target)
                samples.append(result.record["elapsed_seconds"])
            finally:
                os.utime(
                    trigger_path,
                    ns=(original.st_atime_ns, original.st_mtime_ns),
                )
        return {
            "command": command,
            "trigger": trigger_path.relative_to(ROOT).as_posix(),
            "trigger_method": "mtime-only; original atime and mtime restored in finally",
            "samples_seconds": samples,
            "median_seconds": round(median(samples), 6),
        }

    def rust_timings(self) -> dict[str, dict[str, Any]]:
        TARGET_ROOT.mkdir(parents=True, exist_ok=True)
        run_id = uuid.uuid4().hex[:12]
        build_command = ["cargo", "build", "--workspace", "--timings"]

        clean_targets = [
            TARGET_ROOT / f"clean-workspace-build-{index}-{run_id}"
            for index in range(1, 4)
        ]
        timings = {
            "clean-workspace-build": self.timing_samples(build_command, clean_targets)
        }

        no_op_target = TARGET_ROOT / f"no-op-workspace-build-{run_id}"
        no_op_target.mkdir(parents=True, exist_ok=False)
        self.cargo_in_existing_target(build_command, no_op_target)
        no_op_samples = [
            self.cargo_in_existing_target(build_command, no_op_target).record[
                "elapsed_seconds"
            ]
            for _ in range(3)
        ]
        timings["no-op-workspace-build"] = {
            "command": build_command,
            "samples_seconds": no_op_samples,
            "median_seconds": round(median(no_op_samples), 6),
        }

        incremental_target = TARGET_ROOT / f"incremental-checks-{run_id}"
        incremental_target.mkdir(parents=True, exist_ok=False)
        self.cargo_in_existing_target(
            ["cargo", "check", "--workspace", "--timings"], incremental_target
        )
        triggers = {
            "incremental-frontend-asset-check": BUNDLE_DIR / "app.js",
            "incremental-graphql-check": ROOT / "crates" / "noema-core" / "src" / "graphql" / "schema.rs",
            "incremental-provider-check": ROOT / "crates" / "noema-core" / "src" / "provider.rs",
            "incremental-store-check": ROOT / "crates" / "noema-core" / "src" / "store.rs",
        }
        for name, trigger in triggers.items():
            if not trigger.is_file():
                raise BaselineError(f"incremental trigger is missing: {trigger}")
            timings[name] = self.incremental_samples(name, trigger, incremental_target)
        return timings

    def capture(self) -> dict[str, Any]:
        assert_repository_safety()
        OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
        MEASUREMENT_ROOT.mkdir(parents=True, exist_ok=True)

        cache_before = self.sccache_stats()
        cpu_model = self.cpu_model()
        tools = self.tool_versions()
        repository = self.repository_metadata()
        locks = lock_inventory()

        frontend_build = self.command(["bun", "run", "build"], cwd=WEB_DIR)
        bundle = inventory_bundle(BUNDLE_DIR)
        frontend_build.record["output_artifacts"] = [
            _artifact_record(BUNDLE_DIR / asset["path"]) for asset in bundle
        ]

        timings = self.rust_timings()
        cache_after = self.sccache_stats()
        return {
            "schema_version": 1,
            "captured_at": datetime.now(timezone.utc)
            .replace(microsecond=0)
            .isoformat()
            .replace("+00:00", "Z"),
            "repository": repository,
            "environment": {
                "os": platform.platform(),
                "architecture": platform.machine(),
                "cpu_model": cpu_model,
                "tools": tools,
            },
            "locks": locks,
            "cache": {
                "policy": {
                    "cargo_rustc_wrapper": ".cargo/rustc-wrapper",
                    "compiler_cache": "sccache",
                    "allowed_override": "CARGO_TARGET_DIR only",
                    "wrapper_sha256": sha256_file(ROOT / ".cargo" / "rustc-wrapper"),
                },
                "before": cache_before,
                "after": cache_after,
            },
            "frontend_bundle": bundle,
            "rust_timings": timings,
            "commands": self.commands,
        }


def lock_inventory() -> dict[str, dict[str, Any]]:
    """Capture required lock inputs and any source-tree Swift package locks."""

    paths = [
        ROOT / "Cargo.lock",
        ROOT / "crates" / "noema-core" / "web" / "bun.lock",
        ROOT / "crates" / "noema-core" / "mnemosyne-sidecar" / "pyproject.toml",
    ]
    for candidate in ROOT.rglob("Package.resolved"):
        relative_parts = candidate.relative_to(ROOT).parts
        if not {".git", ".local", "node_modules", "target", ".build"}.intersection(
            relative_parts
        ):
            paths.append(candidate)
    inventory = {}
    for path in sorted(set(paths)):
        relative = path.relative_to(ROOT).as_posix()
        inventory[relative] = {
            "present": path.is_file(),
            "sha256": sha256_file(path) if path.is_file() else None,
        }
    return inventory


def assert_repository_safety() -> None:
    """Verify the repository still enforces the required cache/wrapper policy."""

    config = (ROOT / ".cargo" / "config.toml").read_text(encoding="utf-8")
    wrapper = (ROOT / ".cargo" / "rustc-wrapper").read_text(encoding="utf-8")
    if 'rustc-wrapper = ".cargo/rustc-wrapper"' not in config:
        raise BaselineError(".cargo/config.toml must use .cargo/rustc-wrapper")
    if 'exec sccache "$@"' not in wrapper:
        raise BaselineError(".cargo/rustc-wrapper must delegate to sccache")


def quick_check() -> None:
    """Run non-building safety and deterministic aggregation checks."""

    assert_repository_safety()
    if ROOT != Path.cwd().resolve():
        raise BaselineError("run this script from the repository root")
    median([1.0, 2.0, 3.0])
    render_json({"baseline": True})
    if BUNDLE_DIR.exists():
        inventory_bundle(BUNDLE_DIR)


def write_baseline(baseline: Mapping[str, Any]) -> None:
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    JSON_PATH.write_text(render_json(baseline), encoding="utf-8")
    MARKDOWN_PATH.write_text(render_markdown(baseline), encoding="utf-8")


def parse_args(argv: Optional[Sequence[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--quick-check", action="store_true")
    mode.add_argument("--capture", action="store_true")
    return parser.parse_args(argv)


def main(argv: Optional[Sequence[str]] = None) -> int:
    args = parse_args(argv)
    try:
        if args.quick_check:
            quick_check()
            print("foundation baseline quick check passed")
        else:
            baseline = BaselineCapture().capture()
            write_baseline(baseline)
            print(f"wrote {JSON_PATH.relative_to(ROOT)}")
            print(f"wrote {MARKDOWN_PATH.relative_to(ROOT)}")
    except BaselineError as error:
        print(f"foundation baseline error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
