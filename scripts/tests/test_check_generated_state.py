import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
CHECKER = REPOSITORY_ROOT / "scripts" / "check_generated_state.py"
GENERATED_PATHS = (
    Path("crates/noema-core/web/src/generated/schema.graphql"),
    Path("crates/noema-core/web/src/generated/graphql.ts"),
    Path("crates/noema-core/web/src/routeTree.gen.ts"),
)
ASSET_ROOT = Path("crates/noema-core/target/web-assets")


class GeneratedStateCheckerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.repository = Path(self.temporary_directory.name)

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def run_checker(self, mode: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(CHECKER),
                mode,
                "--repo-root",
                str(self.repository),
            ],
            check=False,
            capture_output=True,
            text=True,
        )

    def write_file(self, path: Path, contents: str = "generated\n") -> None:
        target = self.repository / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(contents, encoding="utf-8")

    def initialize_clean_generated_repository(self) -> None:
        subprocess.run(
            ["git", "init", "--quiet", str(self.repository)],
            check=True,
            capture_output=True,
            text=True,
        )
        for path in GENERATED_PATHS:
            self.write_file(path)
        subprocess.run(
            ["git", "-C", str(self.repository), "add", *map(str, GENERATED_PATHS)],
            check=True,
            capture_output=True,
            text=True,
        )
        subprocess.run(
            [
                "git",
                "-C",
                str(self.repository),
                "-c",
                "user.name=Noema Tests",
                "-c",
                "user.email=noema-tests@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "generated baseline",
            ],
            check=True,
            capture_output=True,
            text=True,
        )

    def write_valid_assets(self) -> None:
        self.write_file(
            ASSET_ROOT / "index.html",
            """<!doctype html>
<html>
  <head><link rel="stylesheet" href="/assets/styles.css"></head>
  <body><script type="module" src="/assets/app.js"></script></body>
</html>
""",
        )
        self.write_file(ASSET_ROOT / "app.js", "console.log('noema');\n")
        self.write_file(ASSET_ROOT / "styles.css", "body { color: black; }\n")

    def assert_error(self, result: subprocess.CompletedProcess[str], expected: str) -> None:
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(expected, result.stderr)

    def test_accepts_clean_generated_state(self) -> None:
        self.initialize_clean_generated_repository()

        result = self.run_checker("--generated")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("generated state check passed", result.stdout)

    def test_rejects_missing_generated_file(self) -> None:
        self.initialize_clean_generated_repository()
        missing_path = GENERATED_PATHS[0]
        (self.repository / missing_path).unlink()

        result = self.run_checker("--generated")

        self.assert_error(result, f"generated state error: {missing_path}: missing")

    def test_rejects_untracked_generated_file(self) -> None:
        self.initialize_clean_generated_repository()
        untracked_path = GENERATED_PATHS[1]
        subprocess.run(
            [
                "git",
                "-C",
                str(self.repository),
                "rm",
                "--quiet",
                "--cached",
                str(untracked_path),
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        subprocess.run(
            [
                "git",
                "-C",
                str(self.repository),
                "-c",
                "user.name=Noema Tests",
                "-c",
                "user.email=noema-tests@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "untrack generated file",
            ],
            check=True,
            capture_output=True,
            text=True,
        )

        result = self.run_checker("--generated")

        self.assert_error(result, f"generated state error: {untracked_path}: untracked")

    def test_rejects_unstaged_generated_change(self) -> None:
        self.initialize_clean_generated_repository()
        dirty_path = GENERATED_PATHS[1]
        self.write_file(dirty_path, "changed generated output\n")

        result = self.run_checker("--generated")

        self.assert_error(result, f"generated state error: {dirty_path}: dirty")
        self.assertNotIn("changed generated output", result.stderr)

    def test_rejects_staged_generated_change(self) -> None:
        self.initialize_clean_generated_repository()
        dirty_path = GENERATED_PATHS[2]
        self.write_file(dirty_path, "staged generated output\n")
        subprocess.run(
            ["git", "-C", str(self.repository), "add", str(dirty_path)],
            check=True,
            capture_output=True,
            text=True,
        )

        result = self.run_checker("--generated")

        self.assert_error(result, f"generated state error: {dirty_path}: dirty")
        self.assertNotIn("staged generated output", result.stderr)

    def test_rejects_staged_change_when_worktree_matches_head(self) -> None:
        self.initialize_clean_generated_repository()
        dirty_path = GENERATED_PATHS[2]
        self.write_file(dirty_path, "staged generated output\n")
        subprocess.run(
            ["git", "-C", str(self.repository), "add", str(dirty_path)],
            check=True,
            capture_output=True,
            text=True,
        )
        self.write_file(dirty_path, "generated\n")

        result = self.run_checker("--generated")

        self.assert_error(result, f"generated state error: {dirty_path}: dirty")
        self.assertNotIn("staged generated output", result.stderr)

    def test_accepts_valid_assets_with_configured_prefix(self) -> None:
        self.write_valid_assets()

        result = self.run_checker("--assets")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("built asset check passed", result.stdout)

    def test_accepts_current_vite_javascript_reference_shapes(self) -> None:
        self.write_valid_assets()
        self.write_file(
            ASSET_ROOT / "app.js",
            """const __vite__mapDeps=(i,m=__vite__mapDeps,d=(m.f||(m.f=["event.js","route.css"])))=>i.map(i=>d[i]);
const route=()=>import("./route.js");
""",
        )
        self.write_file(
            ASSET_ROOT / "route.js",
            'import{value as shared}from"./shared.js"; export{shared};\n',
        )
        self.write_file(ASSET_ROOT / "shared.js", "export const value = 1;\n")
        self.write_file(
            ASSET_ROOT / "event.js",
            'import{invoke}from"./shared.js"; export{invoke};\n',
        )
        self.write_file(ASSET_ROOT / "route.css", ".route { display: block; }\n")

        result = self.run_checker("--assets")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("built asset check passed", result.stdout)

    def test_rejects_missing_lazy_javascript_chunk(self) -> None:
        self.write_valid_assets()
        self.write_file(ASSET_ROOT / "app.js", 'import("./missing-lazy.js");\n')

        result = self.run_checker("--assets")

        missing_path = ASSET_ROOT / "missing-lazy.js"
        self.assert_error(result, f"built asset error: {missing_path}: missing")

    def test_rejects_empty_lazy_javascript_chunk(self) -> None:
        self.write_valid_assets()
        self.write_file(ASSET_ROOT / "app.js", 'import("./empty-lazy.js");\n')
        self.write_file(ASSET_ROOT / "empty-lazy.js", "")

        result = self.run_checker("--assets")

        empty_path = ASSET_ROOT / "empty-lazy.js"
        self.assert_error(result, f"built asset error: {empty_path}: empty")

    def test_rejects_missing_transitive_javascript_chunk(self) -> None:
        self.write_valid_assets()
        self.write_file(ASSET_ROOT / "app.js", 'import("./route.js");\n')
        self.write_file(
            ASSET_ROOT / "route.js",
            'import{value}from"./missing-shared.js"; export{value};\n',
        )

        result = self.run_checker("--assets")

        missing_path = ASSET_ROOT / "missing-shared.js"
        self.assert_error(result, f"built asset error: {missing_path}: missing")

    def test_traverses_recursive_javascript_chunks(self) -> None:
        self.write_valid_assets()
        self.write_file(ASSET_ROOT / "app.js", 'import("./route.js");\n')
        self.write_file(ASSET_ROOT / "route.js", 'import "./shared.js";\n')
        self.write_file(ASSET_ROOT / "shared.js", "export const value = 1;\n")

        result = self.run_checker("--assets")

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_cycle_protects_javascript_traversal(self) -> None:
        self.write_valid_assets()
        self.write_file(ASSET_ROOT / "app.js", 'import("./route.js");\n')
        self.write_file(ASSET_ROOT / "route.js", 'import "./shared.js";\n')
        self.write_file(ASSET_ROOT / "shared.js", 'import "./route.js";\n')

        result = self.run_checker("--assets")

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_missing_vite_dependency_table_asset(self) -> None:
        self.write_valid_assets()
        self.write_file(
            ASSET_ROOT / "app.js",
            'const __vite__mapDeps=(i,m=__vite__mapDeps,d=(m.f||(m.f=["missing-preload.js"])))=>i.map(i=>d[i]);\n',
        )

        result = self.run_checker("--assets")

        missing_path = ASSET_ROOT / "missing-preload.js"
        self.assert_error(result, f"built asset error: {missing_path}: missing")

    def test_rejects_unsafe_javascript_references_without_echoing_them(self) -> None:
        unsafe_references = (
            ('import("../private/secret.js")', "traversal reference", "private"),
            (
                'import("/absolute/secret.js")',
                "unsupported absolute reference",
                "secret.js",
            ),
            (
                'import("https://cdn.example.invalid/secret.js")',
                "external reference",
                "cdn.example.invalid",
            ),
            ('import("./nested/secret.js")', "invalid reference", "nested"),
            ("import(routeName)", "malformed JavaScript reference", "routeName"),
        )
        app_path = ASSET_ROOT / "app.js"
        for source, reason, attacker_value in unsafe_references:
            with self.subTest(reason=reason):
                self.write_valid_assets()
                self.write_file(app_path, source + ";\n")

                result = self.run_checker("--assets")

                self.assert_error(result, f"built asset error: {app_path}: {reason}")
                self.assertNotIn(attacker_value, result.stderr)

    def test_all_checks_generated_state_and_built_assets(self) -> None:
        self.initialize_clean_generated_repository()
        self.write_valid_assets()

        result = self.run_checker("--all")

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("generated state check passed", result.stdout)
        self.assertIn("built asset check passed", result.stdout)

    def test_rejects_each_missing_required_entry(self) -> None:
        for missing_name in ("index.html", "app.js", "styles.css"):
            with self.subTest(missing_name=missing_name):
                self.write_valid_assets()
                (self.repository / ASSET_ROOT / missing_name).unlink()

                result = self.run_checker("--assets")

                missing_path = ASSET_ROOT / missing_name
                self.assert_error(result, f"built asset error: {missing_path}: missing")

    def test_rejects_each_empty_required_entry(self) -> None:
        for empty_name in ("index.html", "app.js", "styles.css"):
            with self.subTest(empty_name=empty_name):
                self.write_valid_assets()
                self.write_file(ASSET_ROOT / empty_name, "")

                result = self.run_checker("--assets")

                empty_path = ASSET_ROOT / empty_name
                self.assert_error(result, f"built asset error: {empty_path}: empty")

    def test_rejects_missing_referenced_file(self) -> None:
        self.write_valid_assets()
        self.write_file(
            ASSET_ROOT / "index.html",
            """<link rel="stylesheet" href="/assets/styles.css">
<script type="module" src="/assets/app.js"></script>
<script src="/assets/missing.js"></script>
""",
        )

        result = self.run_checker("--assets")

        missing_path = ASSET_ROOT / "missing.js"
        self.assert_error(result, f"built asset error: {missing_path}: missing")

    def test_rejects_empty_referenced_file(self) -> None:
        self.write_valid_assets()
        self.write_file(
            ASSET_ROOT / "index.html",
            """<link rel="stylesheet" href="/assets/styles.css">
<script type="module" src="/assets/app.js"></script>
<link rel="icon" href="/assets/empty.svg">
""",
        )
        self.write_file(ASSET_ROOT / "empty.svg", "")

        result = self.run_checker("--assets")

        empty_path = ASSET_ROOT / "empty.svg"
        self.assert_error(result, f"built asset error: {empty_path}: empty")

    def test_rejects_traversal_reference(self) -> None:
        self.write_valid_assets()
        self.write_file(
            ASSET_ROOT / "index.html",
            """<link rel="stylesheet" href="/assets/styles.css">
<script type="module" src="/assets/app.js"></script>
<script src="/assets/../escape.js"></script>
""",
        )

        result = self.run_checker("--assets")

        index_path = ASSET_ROOT / "index.html"
        self.assert_error(result, f"built asset error: {index_path}: traversal reference")

    def test_rejects_absolute_remainder_after_asset_prefix(self) -> None:
        self.write_valid_assets()
        self.write_file(
            ASSET_ROOT / "index.html",
            """<link rel="stylesheet" href="/assets/styles.css">
<script type="module" src="/assets/app.js"></script>
<script src="/assets//absolute/outside.js"></script>
""",
        )

        result = self.run_checker("--assets")

        index_path = ASSET_ROOT / "index.html"
        self.assert_error(result, f"built asset error: {index_path}: invalid reference")
        self.assertNotIn("absolute/outside.js", result.stderr)

    def test_rejects_referenced_symlink_that_resolves_outside_asset_root(self) -> None:
        self.write_valid_assets()
        outside_path = self.repository / "outside.js"
        outside_path.write_text("outside asset\n", encoding="utf-8")
        linked_path = self.repository / ASSET_ROOT / "linked.js"
        linked_path.symlink_to(outside_path)
        self.write_file(
            ASSET_ROOT / "index.html",
            """<link rel="stylesheet" href="/assets/styles.css">
<script type="module" src="/assets/app.js"></script>
<script src="/assets/linked.js"></script>
""",
        )

        result = self.run_checker("--assets")

        linked_relative_path = ASSET_ROOT / "linked.js"
        self.assert_error(
            result,
            f"built asset error: {linked_relative_path}: unsafe file",
        )
        self.assertNotIn(str(outside_path), result.stderr)

    def test_rejects_required_entry_symlink_that_resolves_outside_asset_root(self) -> None:
        self.write_valid_assets()
        outside_path = self.repository / "outside-app.js"
        outside_path.write_text("outside app\n", encoding="utf-8")
        app_path = self.repository / ASSET_ROOT / "app.js"
        app_path.unlink()
        app_path.symlink_to(outside_path)

        result = self.run_checker("--assets")

        app_relative_path = ASSET_ROOT / "app.js"
        self.assert_error(
            result,
            f"built asset error: {app_relative_path}: unsafe file",
        )
        self.assertNotIn(str(outside_path), result.stderr)

    def test_rejects_external_reference(self) -> None:
        self.write_valid_assets()
        self.write_file(
            ASSET_ROOT / "index.html",
            """<link rel="stylesheet" href="/assets/styles.css">
<script type="module" src="https://cdn.example.invalid/app.js"></script>
""",
        )

        result = self.run_checker("--assets")

        index_path = ASSET_ROOT / "index.html"
        self.assert_error(result, f"built asset error: {index_path}: external reference")
        self.assertNotIn("cdn.example.invalid", result.stderr)

    def test_rejects_empty_reference(self) -> None:
        self.write_valid_assets()
        self.write_file(
            ASSET_ROOT / "index.html",
            """<link rel="stylesheet" href="/assets/styles.css">
<script type="module" src="/assets/app.js"></script>
<link rel="icon" href="">
""",
        )

        result = self.run_checker("--assets")

        index_path = ASSET_ROOT / "index.html"
        self.assert_error(result, f"built asset error: {index_path}: empty reference")

    def test_rejects_missing_javascript_entry_reference(self) -> None:
        self.write_valid_assets()
        self.write_file(
            ASSET_ROOT / "index.html",
            '<link rel="stylesheet" href="/assets/styles.css">\n',
        )

        result = self.run_checker("--assets")

        index_path = ASSET_ROOT / "index.html"
        self.assert_error(
            result,
            f"built asset error: {index_path}: missing JavaScript entry reference",
        )

    def test_rejects_missing_stylesheet_reference(self) -> None:
        self.write_valid_assets()
        self.write_file(
            ASSET_ROOT / "index.html",
            '<script type="module" src="/assets/app.js"></script>\n',
        )

        result = self.run_checker("--assets")

        index_path = ASSET_ROOT / "index.html"
        self.assert_error(
            result,
            f"built asset error: {index_path}: missing stylesheet reference",
        )


if __name__ == "__main__":
    unittest.main()
