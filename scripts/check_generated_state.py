#!/usr/bin/env python3
"""Verify committed generated sources and production web assets."""

from argparse import ArgumentParser, Namespace
from html.parser import HTMLParser
from pathlib import Path, PurePosixPath
import re
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
MAX_JAVASCRIPT_FILES = 512
MAX_JAVASCRIPT_FILE_BYTES = 4 * 1024 * 1024
MAX_JAVASCRIPT_TOTAL_BYTES = 32 * 1024 * 1024
MAX_JAVASCRIPT_REFERENCES = 4096
MAX_VITE_TABLE_SCAN_BYTES = 64 * 1024

_DYNAMIC_IMPORT = re.compile(
    r"\bimport\s*\(\s*(?P<quote>['\"])(?P<reference>[^'\"\\\r\n]*\.(?:js|css))"
    r"(?P=quote)\s*\)"
)
_DYNAMIC_IMPORT_START = re.compile(r"\bimport\s*\(")
_STATIC_IMPORT = re.compile(
    r"\bimport\s*(?!\()(?:[^;'\"()]{0,4096}?\bfrom\s*)?"
    r"(?P<quote>['\"])(?P<reference>[^'\"\\\r\n]*\.(?:js|css))(?P=quote)"
)
_VITE_TABLE_START = re.compile(r"\.f\s*=\s*\[")


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


def _javascript_asset_path(reference: str) -> Tuple[Optional[Path], Optional[str]]:
    if reference.startswith("/") and not reference.startswith("//"):
        return None, "unsupported absolute reference"

    referenced_path, error = _referenced_asset_path(reference)
    if error is not None:
        return None, error
    assert referenced_path is not None

    if len(referenced_path.parts) != 1:
        return None, "invalid reference"
    if referenced_path.suffix not in (".js", ".css"):
        return None, "invalid reference"
    return referenced_path, None


def _vite_dependency_references(source: str) -> Tuple[List[str], Optional[str]]:
    marker = source.find("__vite__mapDeps")
    if marker == -1:
        return [], None

    scan_end = min(len(source), marker + MAX_VITE_TABLE_SCAN_BYTES)
    table_start = _VITE_TABLE_START.search(source, marker, scan_end)
    if table_start is None:
        return [], "malformed JavaScript reference"
    table_end = source.find("]", table_start.end(), scan_end)
    if table_end == -1:
        return [], "malformed JavaScript reference"

    references: List[str] = []
    position = table_start.end()
    while position < table_end:
        while position < table_end and source[position].isspace():
            position += 1
        if position == table_end:
            break
        quote = source[position]
        if quote not in ('"', "'"):
            return [], "malformed JavaScript reference"
        closing_quote = source.find(quote, position + 1, table_end)
        if closing_quote == -1 or "\\" in source[position + 1 : closing_quote]:
            return [], "malformed JavaScript reference"
        references.append(source[position + 1 : closing_quote])
        position = closing_quote + 1
        while position < table_end and source[position].isspace():
            position += 1
        if position < table_end:
            if source[position] != ",":
                return [], "malformed JavaScript reference"
            position += 1
    return references, None


def _javascript_references(source: str) -> Tuple[List[str], Optional[str]]:
    dynamic_matches = list(_DYNAMIC_IMPORT.finditer(source))
    dynamic_starts = {match.start() for match in dynamic_matches}
    if any(
        match.start() not in dynamic_starts
        for match in _DYNAMIC_IMPORT_START.finditer(source)
    ):
        return [], "malformed JavaScript reference"

    references = [match.group("reference") for match in dynamic_matches]
    references.extend(
        match.group("reference") for match in _STATIC_IMPORT.finditer(source)
    )
    vite_references, vite_error = _vite_dependency_references(source)
    if vite_error is not None:
        return [], vite_error
    references.extend(vite_references)
    return references, None


def _check_javascript_graph(asset_root: Path, entries: Sequence[Path]) -> List[str]:
    errors: List[str] = []
    pending = list(entries)
    visited: set[Path] = set()
    total_bytes = 0
    reference_count = 0

    while pending:
        referenced_path = pending.pop()
        if referenced_path in visited:
            continue
        if len(visited) >= MAX_JAVASCRIPT_FILES:
            errors.append(
                _asset_error(
                    ASSET_ROOT / referenced_path, "JavaScript graph limit exceeded"
                )
            )
            break
        visited.add(referenced_path)

        relative_path = ASSET_ROOT / referenced_path
        path = asset_root / referenced_path
        file_error = _asset_file_error(asset_root, path)
        if file_error is not None:
            errors.append(_asset_error(relative_path, file_error))
            continue
        if referenced_path.suffix != ".js":
            continue

        file_bytes = path.stat().st_size
        total_bytes += file_bytes
        if (
            file_bytes > MAX_JAVASCRIPT_FILE_BYTES
            or total_bytes > MAX_JAVASCRIPT_TOTAL_BYTES
        ):
            errors.append(_asset_error(relative_path, "JavaScript graph limit exceeded"))
            break
        try:
            source = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            errors.append(
                _asset_error(relative_path, "malformed JavaScript reference")
            )
            continue

        references, reference_error = _javascript_references(source)
        if reference_error is not None:
            errors.append(_asset_error(relative_path, reference_error))
            continue
        reference_count += len(references)
        if reference_count > MAX_JAVASCRIPT_REFERENCES:
            errors.append(_asset_error(relative_path, "JavaScript graph limit exceeded"))
            break

        for reference in references:
            child_path, child_error = _javascript_asset_path(reference)
            if child_error is not None:
                errors.append(_asset_error(relative_path, child_error))
                continue
            assert child_path is not None
            pending.append(child_path)

    return errors


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
    javascript_entries: List[Path] = []
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
            javascript_entries.append(referenced_path)
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

    errors.extend(_check_javascript_graph(asset_root, javascript_entries))

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
