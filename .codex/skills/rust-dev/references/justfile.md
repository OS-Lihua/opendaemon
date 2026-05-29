# 用 `justfile` 替代 Makefile（项目级命令组织）

> 目标：统一“开发/测试/门禁/脚本”入口，减少散落的 shell 片段。

## 为什么用 just？

- 语法简单、跨平台（模板对 Windows/Unix 都给了 recipe）
- 参数化与 alias 方便做“命令门面”
- 让项目门禁命令可发现：`just --list`

## 推荐的最小命令集（Rust workspace）

建议至少包含：

- `fmt`：`cargo fmt --all`
- `check`：`cargo check --workspace --all-targets`
- `test`：`cargo test --workspace --all-targets`
- `clippy`：`cargo clippy --workspace --all-targets -- -D warnings`
- `build`：`cargo build --workspace --all-targets --release`
- `size-gate`：文件/函数体门禁（见 `references/size-gates.md`）
- `e2e`：pytest/behave（二选一或都保留）

## 与模板对齐（推荐直接复用模板 justfile）

模板已经提供了较完整的 `justfile`（含 `cargo-nextest`、`uv`、`pytest`、预留
behave）：

- `template/justfile`

建议在项目中保留模板结构，只做增量修改（而不是另起炉灶）。

## 常见实践建议

- `lint`/`happy` 这种“综合门禁”命令作为合并前入口
- CI 中直接跑 `just lint && just test`，本地也保持一致
- 复杂脚本放 `scripts/`，just 只做入口（别把复杂 shell 写进 recipe）

## 参考与来源

- `just` 官方仓库与文档：https://github.com/casey/just
- 模板 justfile：`template/justfile`
