#!/usr/bin/env python3
"""Verify committed generated sources and production web assets."""

from argparse import ArgumentParser, Namespace
from html.parser import HTMLParser
from pathlib import Path, PurePosixPath
import subprocess
import sys
from typing import List, Optional, Sequence, Tuple
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]
GENERATED_PATHS = (
    Path("crates/noema-core/web/src/generated/schema.graphql"),
    Path("crates/noema-core/web/src/generated/graphql.ts"),
    Path("crates/noema-core/web/src/routeTree.gen.ts"),
)
ASSET_ROOT = Path("crates/noema-core/target/web-assets")
REQUIRED_ASSETS = ("index.html", "app.js", "styles.css")
ASSET_URL_PREFIX = "/assets/"


class _ReferenceParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.references: List[Tuple[str, str, Optional[str], Tuple[str, ...]]] = []

    def handle_starttag(
        self, tag: str, attrs: List[Tuple[str, Optional[str]]]
    ) -> None:
        attributes = dict(attrs)
        rel = tuple((attributes.get("rel") or "").lower().split())
        for name in ("src", "href"):
            if name in attributes:
                self.references.append((tag.lower(), name, attributes[name], rel))


def _git_succeeds(repo_root: Path, arguments: Sequence[str]) -> bool:
    result = subprocess.run(
        ["git", "-C", str(repo_root), *arguments],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    return result.returncode == 0


def check_generated_state(repo_root: Path) -> List[str]:
    errors: List[str] = []
    for relative_path in GENERATED_PATHS:
        path = repo_root / relative_path
        if not path.is_file():
            errors.append(f"generated state error: {relative_path}: missing")
            continue
        if not _git_succeeds(
            repo_root, ["ls-files", "--error-unmatch", "--", str(relative_path)]
        ):
            errors.append(f"generated state error: {relative_path}: untracked")
            continue
        index_is_dirty = not _git_succeeds(
            repo_root,
            ["diff", "--cached", "--quiet", "HEAD", "--", str(relative_path)],
        )
        worktree_is_dirty = not _git_succeeds(
            repo_root, ["diff", "--quiet", "--", str(relative_path)]
        )
        if index_is_dirty or worktree_is_dirty:
            errors.append(f"generated state error: {relative_path}: dirty")
    return errors


def _asset_error(relative_path: Path, reason: str) -> str:
    return f"built asset error: {relative_path}: {reason}"


def _referenced_asset_path(reference: str) -> Tuple[Optional[Path], Optional[str]]:
    try:
        parsed = urlsplit(reference)
    except ValueError:
        return None, "invalid reference"

    if parsed.scheme or parsed.netloc or reference.startswith("//"):
        return None, "external reference"

    url_path = unquote(parsed.path)
    if not url_path:
        return None, "empty reference"
    if "\\" in url_path or ".." in PurePosixPath(url_path).parts:
        return None, "traversal reference"

    if url_path.startswith(ASSET_URL_PREFIX):
        url_path = url_path[len(ASSET_URL_PREFIX) :]
        if PurePosixPath(url_path).is_absolute():
            return None, "invalid reference"
    elif url_path.startswith("/"):
        return None, "unsupported absolute reference"

    parts = PurePosixPath(url_path).parts
    if not parts or parts == (".",):
        return None, "empty reference"
    return Path(*parts), None


def _asset_file_error(asset_root: Path, candidate: Path) -> Optional[str]:
    resolved_root = asset_root.resolve()
    resolved_candidate = candidate.resolve()
    try:
        resolved_candidate.relative_to(resolved_root)
    except ValueError:
        return "unsafe file"

    if candidate.is_symlink():
        return "unsafe file"
    if not candidate.is_file():
        return "missing"
    if candidate.stat().st_size == 0:
        return "empty"
    return None


def check_built_assets(repo_root: Path) -> List[str]:
    errors: List[str] = []
    asset_root = repo_root / ASSET_ROOT
    index_error: Optional[str] = None

    for name in REQUIRED_ASSETS:
        relative_path = ASSET_ROOT / name
        path = repo_root / relative_path
        file_error = _asset_file_error(asset_root, path)
        if file_error is not None:
            errors.append(_asset_error(relative_path, file_error))
        if name == "index.html":
            index_error = file_error

    index_path = asset_root / "index.html"
    if index_error is not None:
        return errors

    parser = _ReferenceParser()
    parser.feed(index_path.read_text(encoding="utf-8"))
    has_javascript_entry = False
    has_stylesheet_entry = False
    index_relative_path = ASSET_ROOT / "index.html"

    for tag, attribute, reference, rel in parser.references:
        if reference is None or not reference.strip():
            errors.append(_asset_error(index_relative_path, "empty reference"))
            continue

        referenced_path, reference_error = _referenced_asset_path(reference.strip())
        if reference_error is not None:
            errors.append(_asset_error(index_relative_path, reference_error))
            continue
        assert referenced_path is not None

        if tag == "script" and attribute == "src" and referenced_path.suffix == ".js":
            has_javascript_entry = True
        if (
            tag == "link"
            and attribute == "href"
            and "stylesheet" in rel
            and referenced_path.suffix == ".css"
        ):
            has_stylesheet_entry = True

        relative_path = ASSET_ROOT / referenced_path
        path = repo_root / relative_path
        file_error = _asset_file_error(asset_root, path)
        if file_error is not None:
            errors.append(_asset_error(relative_path, file_error))

    if not has_javascript_entry:
        errors.append(
            _asset_error(index_relative_path, "missing JavaScript entry reference")
        )
    if not has_stylesheet_entry:
        errors.append(
            _asset_error(index_relative_path, "missing stylesheet reference")
        )

    return list(dict.fromkeys(errors))


def _parse_args() -> Namespace:
    parser = ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--generated", action="store_true")
    mode.add_argument("--assets", action="store_true")
    mode.add_argument("--all", action="store_true")
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    return parser.parse_args()


def main() -> int:
    args = _parse_args()
    repo_root = args.repo_root.resolve()
    errors: List[str] = []

    if args.generated or args.all:
        generated_errors = check_generated_state(repo_root)
        errors.extend(generated_errors)
        if not generated_errors:
            print("generated state check passed")

    if args.assets or args.all:
        asset_errors = check_built_assets(repo_root)
        errors.extend(asset_errors)
        if not asset_errors:
            print("built asset check passed")

    for error in errors:
        print(error, file=sys.stderr)
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
