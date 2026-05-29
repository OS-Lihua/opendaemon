---
name: rust-testing
description: |
  Rust 项目测试最佳实践：如何组织单元/集成测试，并使用 mockall（trait mock）、proptest（property-based fake）、wiremock/mockito（HTTP mock）解耦外部依赖，覆盖边缘条件与特殊情况，同时保持 cargo fmt/test/clippy -D warnings 全绿。
---

# Rust Testing（通用）最佳实践 Skill

## 适用场景（何时触发）

当你需要做以下工作时使用本 skill：

- 为某个模块补齐/重构 **单元测试**、**集成测试**、（可选但推荐）端到端测试。
- 需要用 **mockall** 对 trait/服务进行 mock，断言调用次数、参数与行为。
- 需要用 **proptest** 做 property-based 测试，系统化覆盖边缘条件与组合爆炸。
- 需要用 **wiremock/mockito** 对 HTTP 客户端/服务端进行 mock，避免真实网络依赖。
- 需要把测试按项目规范放到 **`tests/`（标准集成测试）**或 **crate
  内测试目录**（避免 inline tests）。

## 核心目标

- **可维护**：测试组织结构清晰、可扩展，不依赖实现细节。
- **可复现**：随机性、时间、外部依赖可控。
- **覆盖边界**：空输入、0 权重、fallback、错误映射、sticky 写回等特殊路径。
- **持续可构建**：在项目中保证 `cargo fmt` / `cargo test` /
  `cargo clippy -- -D warnings` 通过。

## 快速工作流（推荐）

1. **确定测试级别**
   - 单元测试：验证纯逻辑、算法分支、边界条件。
   - 集成测试：验证模块间协作（service -> repository/client
     等），尤其是“参数透传/错误映射/副作用写回”。
   - e2e：只在必须时使用，且依赖必须 mock（HTTP/DB/Redis 等）。

2. **按目录结构放置测试（强制）**
   - 避免把测试写在生产文件里（inline tests）。
   - 优先选择明确的测试目录结构（见下文的“tests/ vs crate 内测试”）。

3. **选择合适的测试工具**
   - **mockall**：mock trait + 行为断言。
   - **proptest**：生成 fake 输入覆盖边界与组合。
   - **wiremock/mockito**：HTTP mock（只在确实存在 HTTP 依赖时）。

4. **最后统一校验**
   - `cargo fmt`
   - `cargo test`
   - `cargo clippy -- -D warnings`

## 目录结构规范（通用）

Rust 生态里常见的两种风格：

- **风格 A：`tests/`（标准 integration tests）**
  - 更“黑盒”，以 public API 为主
  - 适合 library/crate 对外 API 稳定、你不希望测试依赖私有实现

- **风格 B：crate 内测试（白盒）**
  - `src/tests/`（或 `src/<module>.rs` + `#[cfg(test)] mod tests;`）
  - 更“白盒”，可使用 crate 内部模块/私有 helper
  - 适合快速迭代、重构频繁、需要更细粒度断言的阶段

详见：

- `references/test-layout.md`

补充：

- async 测试：`references/async-testing.md`
- 可复现/稳定性：`references/determinism-and-stability.md`
- 外部依赖解耦/DI：`references/dependency-injection.md`
- 常见坑排查：`references/common-pitfalls.md`

示例代码（建议先看）：

- `references/examples.md`

## Mock / Fake / HTTP Mock 的使用准则

### mockall (trait mock)

适用：

- service/DAO/外部资源访问接口（例如 `NodeQueryService`、session store、HTTP
  client wrapper).

要点：

- 只 mock 边界（I/O），不要 mock 被测逻辑本身。
- 断言关键调用：`times(n)`、`returning(...)`，必要时在 closure 中验证输入参数。
- 避免过度指定（例如每个字段都断言），只断言与需求直接相关的字段。

详见：

- `references/mockall.md`

### proptest (property-based fake)

适用：

- 边界条件多、组合爆炸的逻辑：fallback 链、多 hop 拼接、随机选择约束等。

要点：

- 定义“业务不变量”（invariants）并对输入空间做约束。
- 为复杂结构写自定义 `Strategy`，避免生成大量无效样本。
- 控制 `cases` 数量，保证 CI 速度。

详见：

- `references/proptest.md`

### wiremock / mockito (HTTP mock)

适用：

- 被测逻辑依赖 HTTP 客户端/服务端交互，且不希望引入真实网络。

要点：

- 建议优先 **wiremock**（matchers 丰富、可记录请求、适配 async）。
- 将 base_url/transport 从生产代码中注入（可配置），避免 hardcode。

详见：

- `references/http-mock.md`

## 项目内落地（可选）

如果你在某个具体项目里已经有既定的目录结构与测试风格，可以单独维护一个“项目示例导航”文件。

项目示例导航（可选）：

- 你可以在具体项目中维护一个示例导航文件（例如：`references/project-examples.md`）。
- 本仓库也提供一个项目专用示例：`references/proxy-platform-examples.md`（仅供参考）。

## 参考与来源

- Skill 结构与写作原则（Anatomy of a Skill / Progressive disclosure）来源：
  - https://github.com/anthropics/skills
  - https://github.com/anthropics/skills/tree/main/skills/skill-creator

- wiremock API 示例来源：
  - https://docs.rs/wiremock/latest/wiremock/struct.Mock.html
  - https://docs.rs/wiremock/latest/wiremock/struct.MockServer.html
