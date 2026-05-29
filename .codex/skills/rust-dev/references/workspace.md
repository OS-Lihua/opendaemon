# Workspace 组织（更清晰的“分层 + 多 crate”）

> 目标：依赖图单向、边界明确、统一配置；每个 crate 专注自己的职责。

## 1) 推荐的分层（domain/app/infra/adapters）

把“边界”画清楚，再写代码：

- `domain`：业务实体/规则/纯逻辑；定义端口（traits）。
- `app`：usecase/编排；依赖 `domain`；通过 trait 调用外部能力。
- `infra-*`：外部能力实现（DB/HTTP/FS 等）；实现 `domain/app` 定义的 trait。
- `adapters-*`：入口/协议适配（HTTP/gRPC/CLI）；依赖 `app`；负责
  wiring（依赖注入/配置装配）。

硬约束（依赖方向）：

- `domain` 不能依赖 `infra*/adapters*`
- `app` 不能依赖 `adapters*`
- `adapters*` 可以依赖 `infra*`（用于装配），但不要把 infra 类型泄漏到
  app/domain

## 2) 推荐的目录结构（示例）

```text
Cargo.toml                # workspace root
rust-toolchain.toml        # 可选：锁定工具链
rustfmt.toml               # 统一格式化
justfile                   # 项目命令入口（替代 Makefile）
crates/
  domain/
  app/
  infra-db/
  infra-http/
  adapters-http/
  adapters-cli/
  xtask/                   # 可选：自定义任务（例如 codegen/发布检查）
```

## 3) workspace 根 `Cargo.toml`（关键字段）

最低限度建议：

```toml
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.package]
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
anyhow = "1"
serde = { version = "1", features = ["derive"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

然后每个 crate 里用 `workspace = true` 继承依赖版本，避免版本漂移：

```toml
[dependencies]
anyhow = { workspace = true }
serde = { workspace = true }
tokio = { workspace = true }
```

## 4) 依赖治理（让依赖图“看得懂”）

建议在这些场景做依赖治理：

- 依赖冲突/重复依赖变多：先跑 `cargo tree -d` 找重复，再统一版本/feature。
- feature 爆炸：把 feature 聚合到边界 crate（例如 adapters），不要让 domain 参与
  feature 组合。
- 出现循环依赖：直接重切分 crate 边界；不要靠 `pub use`、`mod` 叠罗汉硬压。

## 5) workspace 的统一配置（建议）

- `rustfmt.toml`：统一格式化，避免风格分裂。
- `clippy` 门禁：`cargo clippy --workspace --all-targets -- -D warnings`。
- `profile.release`：性能测量时按需打开行号信息（便于 profiling 定位）。

## 参考与来源

- Cargo Book:
  Workspaces：https://doc.rust-lang.org/cargo/reference/workspaces.html
- Cargo Reference: Inheriting a dependency from a
  workspace：https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#inheriting-a-dependency-from-a-workspace
- Rust Performance
  Book（profiling/测量优先）：https://nnethercote.github.io/perf-book/profiling.html
