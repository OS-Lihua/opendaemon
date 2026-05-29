#!/usr/bin/env python3

from __future__ import annotations

import argparse
import fnmatch
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Iterator


DEFAULT_WARN_FILE_LINES = 600
DEFAULT_MAX_FILE_LINES = 800


@dataclass(frozen=True)
class FileFinding:
    relpath: str
    lines: int


@dataclass(frozen=True)
class FunctionFinding:
    relpath: str
    name: str
    start_line: int
    body_lines: int


def _iter_candidate_files(root: Path, patterns: list[str]) -> Iterator[Path]:
    for pattern in patterns:
        yield from root.glob(pattern)


def _is_hidden_or_build_artifact(path: Path) -> bool:
    parts = set(path.parts)
    return (
        ".git" in parts
        or "target" in parts
        or ".idea" in parts
        or ".vscode" in parts
        or "__pycache__" in parts
    )


def _matches_any_glob(relpath: str, globs: Iterable[str]) -> bool:
    return any(fnmatch.fnmatch(relpath, g) for g in globs)


def _read_lines(path: Path) -> list[str]:
    return path.read_text(encoding="utf-8", errors="replace").splitlines()


def _count_file_lines(path: Path) -> int:
    # Use splitlines() to avoid counting a trailing newline as an extra empty line.
    return len(_read_lines(path))


def _iter_rust_functions(lines: list[str]) -> Iterator[tuple[str, int, int]]:
    """
    Extremely lightweight Rust function body size estimator.

    It is intentionally approximate and designed for "giant function" detection:
    - Detects lines containing 'fn '.
    - Skips trait method declarations ending with ';' before any '{'.
    - Counts body lines from the line that opens '{' through the matching closing '}'.

    Known limitations:
    - Macros / strings / comments with braces can confuse brace tracking.
    - Some complex signatures may be missed.
    """

    index = 0
    while index < len(lines):
        line = lines[index]
        if "fn " not in line:
            index += 1
            continue

        # Heuristic name extraction: fn <ident>
        fn_pos = line.find("fn ")
        after = line[fn_pos + 3 :]
        name = ""
        for ch in after:
            if ch == "_" or ch.isalnum():
                name += ch
            else:
                break
        if not name:
            index += 1
            continue

        start_line = index + 1  # 1-based

        # Scan forward to find either '{' (definition) or ';' (declaration).
        scan_index = index
        body_open_line = -1
        while scan_index < len(lines):
            scan_line = lines[scan_index]
            semicolon = scan_line.find(";")
            brace = scan_line.find("{")
            if semicolon != -1 and (brace == -1 or semicolon < brace):
                body_open_line = -1
                break
            if brace != -1:
                body_open_line = scan_index
                break
            scan_index += 1

        if body_open_line == -1:
            index += 1
            continue

        brace_depth = 0
        end_index = body_open_line
        for j in range(body_open_line, len(lines)):
            brace_depth += lines[j].count("{")
            brace_depth -= lines[j].count("}")
            if brace_depth == 0:
                end_index = j
                break

        body_lines = (end_index - body_open_line) + 1
        yield (name, start_line, body_lines)

        index = max(index + 1, end_index + 1)


def _format_table(rows: list[list[str]]) -> str:
    if not rows:
        return ""
    widths = [max(len(r[i]) for r in rows) for i in range(len(rows[0]))]
    rendered = []
    for row in rows:
        rendered.append("  ".join(cell.ljust(widths[i]) for i, cell in enumerate(row)))
    return "\n".join(rendered)


def _parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="rust_size_gate.py",
        description=(
            "Rust 单文件/函数体行数门禁检查（支持 glob）。默认红牌失败（>max），黄牌仅提示（>warn）。"
        ),
    )
    parser.add_argument(
        "--root",
        default=".",
        help="项目根目录（默认：当前目录）。",
    )
    parser.add_argument(
        "--glob",
        dest="globs",
        action="append",
        required=True,
        help="要扫描的 glob（可重复），例如：crates/**/*.rs",
    )
    parser.add_argument(
        "--exclude-glob",
        dest="exclude_globs",
        action="append",
        default=[],
        help="排除 glob（可重复），例如：**/generated/**",
    )
    parser.add_argument(
        "--warn-file-lines",
        type=int,
        default=DEFAULT_WARN_FILE_LINES,
        help=f"单文件黄牌阈值（默认：{DEFAULT_WARN_FILE_LINES}）。",
    )
    parser.add_argument(
        "--max-file-lines",
        type=int,
        default=DEFAULT_MAX_FILE_LINES,
        help=f"单文件红牌阈值（默认：{DEFAULT_MAX_FILE_LINES}）。",
    )
    parser.add_argument(
        "--warn-fn-lines",
        type=int,
        default=None,
        help="函数体黄牌阈值（可选）。",
    )
    parser.add_argument(
        "--max-fn-lines",
        type=int,
        default=None,
        help="函数体红牌阈值（可选）。",
    )
    parser.add_argument(
        "--fail-on-warn",
        action="store_true",
        help="将黄牌视为失败（CI 强制时使用）。",
    )
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = _parse_args(argv)

    root = Path(args.root).resolve()
    if not root.exists() or not root.is_dir():
        print(f"[ERROR] --root 不是有效目录：{root}", file=sys.stderr)
        return 2

    candidates = sorted(set(_iter_candidate_files(root, args.globs)))
    files: list[Path] = []
    for p in candidates:
        if not p.is_file():
            continue
        if _is_hidden_or_build_artifact(p.relative_to(root)):
            continue
        rel = p.relative_to(root).as_posix()
        if _matches_any_glob(rel, args.exclude_globs):
            continue
        files.append(p)

    file_warnings: list[FileFinding] = []
    file_errors: list[FileFinding] = []

    fn_warnings: list[FunctionFinding] = []
    fn_errors: list[FunctionFinding] = []

    for path in files:
        rel = path.relative_to(root).as_posix()
        line_count = _count_file_lines(path)
        finding = FileFinding(relpath=rel, lines=line_count)
        if line_count > args.max_file_lines:
            file_errors.append(finding)
        elif line_count > args.warn_file_lines:
            file_warnings.append(finding)

        if args.warn_fn_lines is None and args.max_fn_lines is None:
            continue

        warn_fn = args.warn_fn_lines if args.warn_fn_lines is not None else -1
        max_fn = args.max_fn_lines if args.max_fn_lines is not None else -1
        lines = _read_lines(path)
        for name, start_line, body_lines in _iter_rust_functions(lines):
            fn_finding = FunctionFinding(
                relpath=rel,
                name=name,
                start_line=start_line,
                body_lines=body_lines,
            )
            if max_fn != -1 and body_lines > max_fn:
                fn_errors.append(fn_finding)
            elif warn_fn != -1 and body_lines > warn_fn:
                fn_warnings.append(fn_finding)

    def _print_section(title: str, rows: list[list[str]]) -> None:
        if not rows:
            return
        print()
        print(title)
        print(_format_table(rows))

    print(f"[INFO] root={root}")
    print(f"[INFO] scanned_files={len(files)}")
    print(f"[INFO] file_warn>{args.warn_file_lines} file_max>{args.max_file_lines}")
    if args.warn_fn_lines is not None or args.max_fn_lines is not None:
        print(f"[INFO] fn_warn>{args.warn_fn_lines} fn_max>{args.max_fn_lines}")

    file_warn_rows = [[str(f.lines), f.relpath] for f in sorted(file_warnings, key=lambda x: (-x.lines, x.relpath))]
    file_err_rows = [[str(f.lines), f.relpath] for f in sorted(file_errors, key=lambda x: (-x.lines, x.relpath))]
    _print_section("[WARN] oversized files", file_warn_rows)
    _print_section("[ERROR] oversized files", file_err_rows)

    fn_warn_rows = [
        [str(f.body_lines), f"{f.relpath}:{f.start_line}", f.name]
        for f in sorted(fn_warnings, key=lambda x: (-x.body_lines, x.relpath, x.start_line, x.name))
    ]
    fn_err_rows = [
        [str(f.body_lines), f"{f.relpath}:{f.start_line}", f.name]
        for f in sorted(fn_errors, key=lambda x: (-x.body_lines, x.relpath, x.start_line, x.name))
    ]
    _print_section("[WARN] oversized functions (approx)", fn_warn_rows)
    _print_section("[ERROR] oversized functions (approx)", fn_err_rows)

    warn_count = len(file_warnings) + len(fn_warnings)
    err_count = len(file_errors) + len(fn_errors)
    print()
    print(f"[SUMMARY] warnings={warn_count} errors={err_count}")

    if err_count > 0:
        return 2
    if args.fail_on_warn and warn_count > 0:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
