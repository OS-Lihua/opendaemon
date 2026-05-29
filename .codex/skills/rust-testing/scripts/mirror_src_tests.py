"""mirror_src_tests.py

用途：
- 帮你快速生成 crate 内 `src/tests` 的模块骨架（不直接改文件，输出到 stdout）。
- 适合在你要把 inline tests 迁移到 `src/tests` 时使用。

用法：
  python scripts/mirror_src_tests.py --crate /path/to/your-crate

说明：
- 只输出建议的 `mod.rs` 结构与文件路径清单。
- 你可以根据输出手动创建文件，或把输出复制到对应 `mod.rs`。

限制：
- 不解析 Rust 语法，仅按文件系统结构生成建议。
"""

from __future__ import annotations

import argparse
from pathlib import Path


def module_parts_from_rs_file(rel: Path) -> tuple[str, ...] | None:
    if rel.suffix != ".rs":
        return None
    if rel.name in {"lib.rs", "main.rs"}:
        return None
    if rel.name == "mod.rs":
        if not rel.parent.parts:
            return None
        return tuple(rel.parent.parts)
    return tuple(rel.with_suffix("").parts)


def build_module_tree(rs_files: list[Path], src_dir: Path) -> dict[str, dict]:
    tree: dict[str, dict] = {}
    for p in rs_files:
        rel = rel_to_src(p, src_dir)
        if rel.parts and rel.parts[0] == "tests":
            continue

        parts = module_parts_from_rs_file(rel)
        if not parts:
            continue

        node = tree
        for part in parts:
            node = node.setdefault(part, {})
    return tree


def iter_nodes(tree: dict[str, dict], prefix: tuple[str, ...] = ()):
    for name in sorted(tree.keys()):
        child = tree[name]
        path = prefix + (name,)
        yield path, child
        if child:
            yield from iter_nodes(child, path)


def is_rs_file(p: Path) -> bool:
    return p.is_file() and p.suffix == ".rs"


def rel_to_src(p: Path, src_dir: Path) -> Path:
    return p.relative_to(src_dir)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--crate", required=True, help="crate 根目录（包含 src/）")
    args = ap.parse_args()

    crate_dir = Path(args.crate)
    src_dir = crate_dir / "src"
    if not src_dir.exists():
        raise SystemExit(f"src dir not found: {src_dir}")

    rs_files = [p for p in src_dir.rglob("*.rs") if is_rs_file(p)]

    tests_dir = src_dir / "tests"

    tree = build_module_tree(rs_files, src_dir)
    top_level_modules = sorted(tree.keys())

    print("# Suggested test tree")
    print(tests_dir)
    print()

    print("# In src/lib.rs:")
    print("#[cfg(test)]")
    print("mod tests;")
    print()

    print("# In src/tests/mod.rs:")
    for m in top_level_modules:
        print(f"mod {m};")

    print()

    print("# Suggested files to create under src/tests:")
    for path, child in iter_nodes(tree):
        if child:
            print(tests_dir / Path(*path) / "mod.rs")
        else:
            print((tests_dir / Path(*path)).with_suffix(".rs"))

    print()
    for path, child in iter_nodes(tree):
        if not child:
            continue
        print(f"# In {tests_dir / Path(*path) / 'mod.rs'}:")
        for name in sorted(child.keys()):
            print(f"mod {name};")
        print()


if __name__ == "__main__":
    main()
