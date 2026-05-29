# 测试总纲（先总结，再指向 rust-testing）

> 目标：用“可复现、可维护”的测试体系覆盖边缘条件；并把测试拆成清晰的 test
> suites。

## 1) 测试金字塔（建议默认）

1. **单元测试（最多）**
   - 只测纯逻辑/算法/边界条件
   - 不依赖网络/磁盘/真实数据库

2. **集成测试（适量）**
   - 测模块协作、错误映射、参数透传、副作用写回
   - 外部依赖用 mock/fake/http-mock 解耦

3. **E2E（少量但关键）**
   - 只覆盖“关键业务链路”，避免把所有细节都堆到 e2e
   - 强制可复现：固定输入、可控时钟、可控网络/服务依赖

## 2) 测试放置规则（强制：避免 inline tests）

优先两种风格之一（按项目阶段选择）：

- `tests/`（integration tests）：更黑盒，围绕 public API。
- crate 内 `src/tests/`：更白盒，适合快速迭代与频繁重构。

不管选哪种，都要与源码形成“镜像结构”（便于定位与扩展）。

## 3) 工具选型（Rust 侧）

按边界选工具：

- trait 边界（I/O 端口）：用 **mockall** 做行为断言
- 输入空间大/组合爆炸：用 **proptest** 覆盖边缘与不变量
- HTTP 依赖：用 **wiremock/mockito**（避免真实网络）

更详细的代码组织方式与示例请直接参考：

- `../../rust-testing/SKILL.md`
- `../../rust-testing/references/test-layout.md`
- `../../rust-testing/references/mockall.md`
- `../../rust-testing/references/proptest.md`
- `../../rust-testing/references/http-mock.md`

## 4) Test suites（强制）

把测试按“速度/稳定性/依赖”拆成 suites，避免 CI 被慢测拖死：

- `unit`：快、纯逻辑（默认每次都跑）
- `integration`：中等（默认每次都跑）
- `e2e`：慢、链路少（按需跑或 nightly 跑）

在 Rust 侧可用：

- `cargo test --workspace --all-targets`
- 若项目引入 `cargo-nextest`，可做更快的 suites 切分（见模板
  `template/justfile`）。

## 5) E2E：BDD（behave）vs 普通（pytest）

如果项目更适合“业务场景/角色/对话式用例”：

- 用 **behave**（BDD），把自然语言场景映射到 step definitions

如果只是一般项目/需要工程化断言：

- 用 **pytest**（参数化、fixture、marker、插件生态更成熟）

无论选择哪种，建议把 e2e 作为独立工程（Python）放在 `e2e/`：

- 参考模板：`e2e/README.md`
- pytest/behave（BDD）更细的落地：`references/e2e-python.md`
- 模板默认使用 `uv` 管理虚拟环境与依赖（也可以改为 `pip/venv`）

## 参考与来源

- `rust-testing` skill（本机路径）：`../../rust-testing/SKILL.md`
- behave 官方文档：https://behave.readthedocs.io/en/stable/
- pytest 官方文档：https://docs.pytest.org/en/stable/
- 模板 e2e 说明：`e2e/README.md`
- `uv` 官方文档：https://docs.astral.sh/uv/
