# 不兼容重构（强制策略）

> 目标：**不为历史包袱做兼容层**。直接迁移到最优结构，迁移完成后删除旧代码。

## 什么时候必须用“不兼容重构”？

- 项目仍处于内部快速迭代阶段（未上生产/可接受破坏性变更）。
- 旧结构已经阻碍开发（循环依赖、巨型模块、职责混乱、测试难写）。
- 兼容层成本会持续增长（每次改动都要维护旧 API/旧路径）。

## 重构的边界定义（先写下来）

强制四层边界：

- `domain` / `app` / `infra` / `adapters`
- 层与层之间只通过 **trait/DTO** 交互

## 推荐迁移步骤（可重复执行）

1. **建新边界（先建新，不动旧）**
   - 新建 crate：`domain`、`app`、`infra-*`、`adapters-*`
   - 在 `domain/app` 定义 ports（traits）与 DTO

2. **迁移一块逻辑（小步快跑）**
   - 把“纯逻辑”先迁移到 `domain`
   - 把“编排逻辑”迁移到 `app`
   - `infra` 实现 `domain/app` 的 trait

3. **补齐测试（迁移即测试）**
   - 纯逻辑：单元测试
   - 编排/边界：集成测试 + mock/fake/http-mock
   - 细节参考 `../../rust-testing/SKILL.md`

4. **替换调用点（一次替换一条链路）**
   - adapters 组装 DI
   - 删除旧 wiring

5. **删旧代码（强制）**
   - 迁移完成的模块：删旧 crate/旧 module/旧路径 alias
   - 禁止保留兼容层“以后再删”

6. **跑全套门禁（强制）**
   - `cargo fmt --all`
   - `cargo check --workspace --all-targets`
   - `cargo test --workspace --all-targets`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo build --workspace --all-targets --release`
   - 文件行数门禁：见 `references/size-gates.md`

## 常见反模式（禁止）

- “先加一个兼容层，等稳定了再删”（通常永远删不掉）
- 旧/新两套实现长期共存且没有迁移计划
- 依赖方向不清导致循环依赖，只能靠 `pub use`/re-export 硬糊

## 参考与来源

- Cargo workspaces（拆 crate
  的基础能力）：https://doc.rust-lang.org/cargo/reference/workspaces.html
- 本项目行数门禁：`references/size-gates.md`
- 测试落地：`../../rust-testing/SKILL.md`
