---
name: rust-project-dev
description: |
  Rust 项目工程化开发 Skill：统一 workspace + 多 crate 分层架构；强制门禁（fmt/check/test/build/clippy -D warnings）；遵循 perf-book 的“先测量再优化”原则；鼓励不兼容重构减债；提供单文件/函数体行数门禁脚本（支持 glob）；测试参考 rust-testing（mockall/proptest/wiremock/mockito，集中管理 tests）。
---

# Rust 项目开发（工程化 + 性能 + 不兼容重构）Skill

## 适用场景（何时触发）

当你需要做以下工作时使用本 skill：

- 新建/重构一个 Rust workspace，希望把项目拆成多个 crates
  并分层（domain/app/infra/adapters）。
- 为一个项目建立**强制门禁**：`fmt/check/test/build/clippy -D warnings` 全绿。
- 需要在不牺牲架构清晰度的前提下做性能优化（遵循 perf-book：先测量/先
  profiling，再优化 hot path）。
- 需要做**不兼容重构**：不保留旧
  API/旧目录结构的兼容层，迁移完成后直接删除旧代码。
- 需要建立“单文件行数 /
  函数体行数”门禁，强制拆文件拆模块，避免巨型文件和巨型函数。
- 需要把测试从 inline tests 迁移到集中结构，并引入 mock/fake/http-mock
  工具链（见 `rust-testing` skill）。

## 核心目标

- **门禁全绿**：`cargo fmt` / `cargo check` / `cargo test` / `cargo build` /
  `cargo clippy -- -D warnings` 全部通过。
- **结构清晰**：workspace + 多 crate 分层，依赖图单向、边界明确（domain 不依赖
  infra）。
- **可组合/可重构**：trait/DTO
  做边界；实现细节可替换（mock/fake），便于重构与测试。
- **性能可解释**：所有优化都来自测量/剖析结论（profiling），只优化 hot code。
- **技术债可控**：不兼容重构优先，迁移后删除旧代码，避免长期兼容层腐化。

## 强制规则（不满足就停止合并/停止交付）

1. **先设计再写代码**：先画清楚分层边界与依赖方向，再动手迁移/实现。
2. **禁止造轮子**：优先检索并选用成熟库；只有在调研后仍不满足需求才自研。
3. **KISS / DRY /
   SOLID**：函数化、模块化、组件化、可组合；避免无效冗余与过度抽象。
4. **不兼容重构（强制）**：不保留旧
   API/旧目录结构兼容层；迁移完成后删除旧模块/旧 crate。
5. **单文件行数门禁**：
   - `> 600` 行：黄牌（必须拆分，允许短期存在但要给出拆分计划）。
   - `> 800` 行：红牌（禁止存在，必须立即拆分）。
6. **Clippy 视为编译门禁的一部分**：`cargo clippy -- -D warnings` 必须全绿（包含
   warnings）。

## 快速工作流（推荐）

> 目标：把“设计 → 实现/迁移 → 测试 → 性能验证 → 门禁全绿”串成可重复的流水线。

### 文档导航（本 skill 的深入细节）

- `references/README.md`

### 0) 统一门禁命令（复制即用）

在 workspace 根目录执行：

```bash
# 1) 格式化（强制）
cargo fmt --all

# 2) 编译检查（快）
cargo check --workspace --all-targets

# 3) 测试（建议包含所有 target）
cargo test --workspace --all-targets

# 4) Clippy（把 warning 当错误）
cargo clippy --workspace --all-targets -- -D warnings

# 5) Release 构建（用于性能/发布验证）
cargo build --workspace --all-targets --release
```

如果需要自动修复（注意：只在你理解修复含义时使用）：

```bash
cargo clippy --workspace --all-targets --fix --allow-dirty -- -D warnings
```

### 1) 分层架构（domain/app/infra/adapters）先行

推荐四层边界（只通过 trait/DTO 交互）：

- **domain**：业务实体/值对象/规则；定义需要的端口（traits）。
- **app**：usecase/编排；依赖 domain；只使用 domain 定义的端口。
- **infra**：端口实现（DB/HTTP/FS 等）；依赖 domain（必要时依赖 app 的
  DTO，但尽量把 DTO 放在 app）。
- **adapters**：入口/协议适配（HTTP/gRPC/CLI/GUI）；依赖 app；组装依赖注入（把
  infra 实现注入 app）。

硬约束（依赖方向）：

- `domain` **不能**依赖 `infra/adapters`。
- `app` **不能**依赖 `adapters`；尽量不直接依赖具体 `infra`（通过 trait 注入）。

### 2) Workspace + 多 crate 管理（模块化与依赖收敛）

建议将大项目拆为多个 crates，各自专注单一职责：

```text
Cargo.toml (workspace root)
crates/
  domain/
  app/
  infra/
  adapters-http/
  adapters-cli/
  xtask/            # 可选：自定义任务（门禁/代码生成/检查）
```

依赖治理建议：

- 用 `[workspace.dependencies]` 统一第三方依赖版本，减少重复依赖与 feature
  分叉。
- 优先让依赖边界发生在 crates 之间，而不是同 crate 内靠 `mod` 堆叠。
- 发现依赖图混乱时先 `cargo tree -d` 查重复依赖，再统一版本/feature。

### 3) 性能优化（perf-book 原则：先测量，再优化 hot code）

按 perf-book 的顺序做事：

1. **先确保你在测量 release**（不要用 debug 做性能结论）。
2. **profiling 找 hot path**：只优化“真正热”的代码。
3. 优先做 **算法/数据结构** 改进，其次才是微优化（小收益会累积，但别盲做）。
4. 针对 hot function：要么让它更快，要么减少它被调用次数。
5. 减少**不必要的计算**、处理常见特殊情况（0/1/2 元素等），并用测量证明价值。

落地建议（可直接抄）：

- 为 release profile 打开行号级 debug info（便于剖析定位）：

```toml
[profile.release]
debug = "line-tables-only"
```

- 需要更好的采样栈时，强制保留 frame pointers：

```bash
RUSTFLAGS="-C force-frame-pointers=yes" cargo build --release
```

### 4) 不兼容重构策略（强制）

目标：**清晰的依赖图 + 删除旧代码**，而不是维护兼容层。

推荐步骤：

1. **定义边界**：domain/app/infra/adapters 四层之间只通过 trait/DTO/接口交互。
2. **把旧模块“抽出来变 crate”**：让编译依赖图清晰，避免“同 crate 巨石”。
3. **逐步迁移逻辑到正确层**：每迁移一块就补测试（单元/集成），并保证门禁全绿。
4. **迁移完成后删除旧 crate/旧模块**：不保留兼容层、不保留旧路径 alias。
5. **跑全套门禁**：见“统一门禁命令”。

### 5) 文件/函数体行数门禁（强制拆分）

本 skill 提供一个 Python 脚本，可对任意项目做门禁检查（支持 glob）：

- 脚本路径：`scripts/rust_size_gate.py`（本仓库位置：`~/.codex/skills/rust-project-dev/scripts/rust_size_gate.py`；

典型用法（在项目根目录执行）：

```bash
python3 ~/.codex/skills/rust-project-dev/scripts/rust_size_gate.py \
  --root . \
  --glob 'crates/**/*.rs' \
  --warn-file-lines 600 \
  --max-file-lines 800
```

可选：对函数体行数也做门禁（粗略统计，主要用于发现“巨型函数”）：

```bash
python3 ~/.codex/skills/rust-project-dev/scripts/rust_size_gate.py \
  --root . \
  --glob 'crates/**/*.rs' \
  --warn-fn-lines 80 \
  --max-fn-lines 150
```

> 建议把它接入 CI / `justfile`：红牌（>800 行）直接失败；黄牌（>600 行）至少在
> PR 描述里给拆分计划。详见：`references/size-gates.md`。

### 6) 测试策略（强制参考 rust-testing）

测试先按“金字塔 + suites”设计，再写具体用例：

- **单元测试（最多）**：纯逻辑/边缘条件/不变量（必要时用 `proptest`）。
- **集成测试（适量）**：模块协作、错误映射、参数透传、副作用写回（外部依赖用
  trait + `mockall`/HTTP mock 解耦）。
- **E2E（少量关键链路）**：建议用独立 `e2e/` 工程；适合 BDD 时用
  `behave`，否则用 `pytest`；强制拆 test suites（smoke/regression/slow/flaky）。

总结版与目录约束见：`references/testing.md`、`references/e2e-python.md`。

更多细节与示例代码请复用 `rust-testing` skill：

- 入口：`../rust-testing/SKILL.md`
- mockall/proptest/http-mock/test layout：`../rust-testing/references/`

## 第三方库调研清单（禁止造轮子）

在写新模块前，至少做一次“快速但严谨”的调研：

1. **明确需求边界**：同步/异步？是否需要
   `no_std`？是否需要可插拔实现？性能/内存约束？
2. **查成熟库**：`crates.io` + `lib.rs` + `docs.rs` + GitHub issues/PR 活跃度。
3. **评估标准**（建议写在 PR 描述里）：
   - 维护活跃度、最近发布、issues 响应
   - API 稳定性与可测试性（是否方便 mock/注入）
   - 依赖体积与 feature 复杂度
   - MSRV（最低 Rust 版本）是否匹配项目
   - License 是否符合项目要求
4. **做最小 POC**：在独立 crate 或 `examples/` 验证关键路径，再引入主干。

调研流程与 ADR 模板见：`references/research.md`。

## 参考与来源

- Skill 结构与写作原则（Anatomy of a Skill / Progressive disclosure）：
  - https://github.com/anthropics/skills
  - https://deepwiki.com/anthropics/skills/6-creating-a-new-skill

- Rust 性能优化原则（perf-book）：
  - https://nnethercote.github.io/perf-book/
  - https://nnethercote.github.io/perf-book/general-tips.html
  - https://nnethercote.github.io/perf-book/profiling.html
  - https://nnethercote.github.io/perf-book/heap-allocations.html
  - https://github.com/nnethercote/perf-book

- Cargo / Clippy / Rustfmt 官方文档：
  - https://doc.rust-lang.org/cargo/commands/cargo-check.html
  - https://doc.rust-lang.org/cargo/commands/cargo-build.html
  - https://doc.rust-lang.org/cargo/commands/cargo-test.html
  - https://doc.rust-lang.org/clippy/usage.html
  - https://github.com/rust-lang/rustfmt

- `just`（项目命令组织）：
  - https://github.com/casey/just
