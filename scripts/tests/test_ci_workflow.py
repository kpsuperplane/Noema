import json
from pathlib import Path
import re
import subprocess
from typing import Any, Dict, List, Optional, Tuple
import unittest


ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = ROOT / ".github" / "workflows" / "ci.yml"
TOOLCHAIN_PATH = ROOT / "rust-toolchain.toml"
WEB_DIRECTORY = "crates/noema-core/web"
SIDECAR_DIRECTORY = "crates/noema-core/mnemosyne-sidecar"

ACTION_PINS = {
    "actions/checkout": ("34e114876b0b11c390a56381ad16ebd13914f8d5", "v4"),
    "actions/setup-python": ("a26af69be951a213d495a4c3e4e4022e16d87065", "v5"),
    "oven-sh/setup-bun": ("0c5077e51419868618aeaa5fe8019c62421857d6", "v2"),
}

PROVIDER_CREDENTIAL_NAMES = (
    "GITHUB_TOKEN",
    "OPENAI_API_KEY",
    "NOEMA_OPENAI__API_KEY",
    "AZURE_OPENAI_API_KEY",
    "CODEX_ACCESS_TOKEN",
    "ANTHROPIC_API_KEY",
    "GOOGLE_API_KEY",
    "GEMINI_API_KEY",
    "EXA_API_KEY",
    "NOEMA_MEMORY_OPENAI_API_KEY",
    "MNEMOSYNE_LLM_API_KEY",
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
)

EXPECTED_STEP_NAMES = [
    "Check out repository",
    "Set up Python",
    "Set up Bun",
    "Activate pinned Rust toolchain",
    "Ensure sccache is available",
    "Install frontend dependencies",
    "Validate frontend",
    "Validate repository scripts",
    "Install sidecar dependencies",
    "Test sidecar",
    "Validate Rust workspace",
    "Verify generated files remain clean",
]

EXPECTED_RUN_LINES: List[Tuple[str, Optional[str]]] = [
    (
        "rustup toolchain install 1.96.0 --profile minimal --component rustfmt "
        "--component clippy",
        None,
    ),
    ("rustup show active-toolchain | grep '^1\\.96\\.0-'", None),
    ("rustc --version", None),
    ("cargo --version", None),
    ("if ! command -v sccache >/dev/null 2>&1; then", None),
    ("brew install sccache", None),
    ("fi", None),
    ("sccache --version", None),
    ("bun install --frozen-lockfile", WEB_DIRECTORY),
    ("bun run check:generated", WEB_DIRECTORY),
    ("bun run test:ci", WEB_DIRECTORY),
    ("bun run lint", WEB_DIRECTORY),
    ("bun run check:assets", WEB_DIRECTORY),
    ("python3 -m unittest discover -s scripts/tests", None),
    ("python3 scripts/check_audit_coverage.py", None),
    ("python3 scripts/check_generated_state.py --all", None),
    ("python -m pip install --upgrade pip", SIDECAR_DIRECTORY),
    ("python -m pip install --editable .", SIDECAR_DIRECTORY),
    ("python -m pip install --group dev", SIDECAR_DIRECTORY),
    ("python -m pytest", SIDECAR_DIRECTORY),
    ("cargo fmt --all --check", None),
    ("cargo check --workspace", None),
    ("cargo clippy --workspace --all-targets -- -D warnings", None),
    ("cargo test --workspace --no-fail-fast", None),
    ("git diff --exit-code", None),
]


def parse_workflow_with_ruby() -> Dict[str, Any]:
    command = (
        "document = YAML.safe_load(File.read(ARGV.fetch(0)), "
        "permitted_classes: [], permitted_symbols: [], aliases: true); "
        "puts JSON.generate(document)"
    )
    result = subprocess.run(
        ["ruby", "-rjson", "-ryaml", "-e", command, str(WORKFLOW_PATH)],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise AssertionError(f"Ruby/Psych rejected ci.yml:\n{result.stderr}")
    parsed = json.loads(result.stdout)
    if not isinstance(parsed, dict):
        raise AssertionError("ci.yml must parse as a mapping")
    return parsed


def run_lines(steps: List[Dict[str, Any]]) -> List[Tuple[str, Optional[str]]]:
    lines: List[Tuple[str, Optional[str]]] = []
    for step in steps:
        run = step.get("run")
        if not isinstance(run, str):
            continue
        working_directory = step.get("working-directory")
        lines.extend(
            (line.strip(), working_directory)
            for line in run.splitlines()
            if line.strip()
        )
    return lines


class CiWorkflowContractTests(unittest.TestCase):
    def test_minimal_foundation_workflow_contract(self) -> None:
        self.assertTrue(WORKFLOW_PATH.is_file(), "CI workflow is missing")
        workflow_text = WORKFLOW_PATH.read_text(encoding="utf-8")
        workflow = parse_workflow_with_ruby()

        self.assertEqual(
            set(workflow), {"name", "on", "permissions", "concurrency", "jobs"}
        )
        self.assertEqual(workflow.get("name"), "CI")
        self.assertEqual(
            workflow.get("on"),
            {
                "pull_request": {},
                "push": {"branches": ["main"]},
                "workflow_dispatch": {},
            },
        )
        self.assertEqual(workflow.get("permissions"), {"contents": "read"})
        self.assertEqual(
            workflow.get("concurrency"),
            {
                "group": "${{ github.workflow }}-${{ github.ref }}",
                "cancel-in-progress": True,
            },
        )

        jobs = workflow.get("jobs")
        self.assertIsInstance(jobs, dict)
        self.assertEqual(list(jobs), ["foundation-validation"])
        job = jobs["foundation-validation"]
        self.assertEqual(set(job), {"runs-on", "timeout-minutes", "steps"})
        self.assertEqual(job.get("runs-on"), "macos-26")
        timeout = job.get("timeout-minutes")
        self.assertIsInstance(timeout, int)
        self.assertGreater(timeout, 0)
        self.assertLessEqual(timeout, 90)
        self.assertNotIn("permissions", job)

        steps = job.get("steps")
        self.assertIsInstance(steps, list)
        self.assertEqual([step.get("name") for step in steps], EXPECTED_STEP_NAMES)
        for step in steps:
            expected_keys = {"name", "uses"}
            if "with" in step:
                expected_keys.add("with")
            if "run" in step:
                expected_keys = {"name", "run"}
                if "working-directory" in step:
                    expected_keys.add("working-directory")
            self.assertEqual(set(step), expected_keys)

        uses = [step.get("uses") for step in steps if "uses" in step]
        self.assertEqual(
            uses,
            [f"{action}@{pin}" for action, (pin, _) in ACTION_PINS.items()],
        )
        for action, (pin, release_tag) in ACTION_PINS.items():
            self.assertRegex(
                workflow_text,
                rf"uses:\s*{re.escape(action)}@{pin}\s+#\s*{release_tag}\b",
            )

        python_step = next(
            step
            for step in steps
            if str(step.get("uses", "")).startswith("actions/setup-python@")
        )
        bun_step = next(
            step
            for step in steps
            if str(step.get("uses", "")).startswith("oven-sh/setup-bun@")
        )
        self.assertEqual(python_step.get("with"), {"python-version": "3.12"})
        self.assertEqual(bun_step.get("with"), {"bun-version": "1.3.14"})

        self.assertTrue(TOOLCHAIN_PATH.is_file(), "Rust toolchain file is missing")
        toolchain_text = TOOLCHAIN_PATH.read_text(encoding="utf-8")
        self.assertRegex(toolchain_text, r'(?m)^channel\s*=\s*"1\.96\.0"\s*$')
        self.assertRegex(toolchain_text, r'(?m)^profile\s*=\s*"minimal"\s*$')
        self.assertRegex(
            toolchain_text,
            r'(?m)^components\s*=\s*\[\s*"rustfmt"\s*,\s*"clippy"\s*\]\s*$',
        )

        self.assertEqual(run_lines(steps), EXPECTED_RUN_LINES)

        lowered = workflow_text.lower()
        self.assertNotIn("secrets.", lowered)
        self.assertNotIn("${{ github.token }}", lowered)
        for credential_name in PROVIDER_CREDENTIAL_NAMES:
            self.assertNotIn(credential_name.lower(), lowered)
        self.assertNotRegex(lowered, r"(?m)^\s*[\w-]+:\s*write\s*(?:#.*)?$")
        self.assertNotRegex(
            lowered,
            r"\b(?:api[_-]?key|access[_-]?token|client[_-]?secret|"
            r"private[_-]?key|credentials?)\b",
        )
        self.assertNotRegex(
            lowered,
            r"\b(?:curl|wget|upload|download|codesign|signing|notarization|"
            r"notarize|notarytool|release|publish)\b",
        )
        for protected_variable in (
            "CARGO_BUILD_RUSTC_WRAPPER",
            "RUSTC_WRAPPER",
            "CC",
            "CXX",
        ):
            self.assertNotIn(protected_variable, workflow_text)


if __name__ == "__main__":
    unittest.main()
