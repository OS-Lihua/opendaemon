# 开发前 / 开发中调研（强制）

> 目标：**不造轮子**、不拍脑袋。所有实现与重构都要有“调研证据”和“可回滚选择”。

## 你应该在什么时候调研？

- **开发前**：开始一个新模块/新能力之前（尤其是：协议、存储、并发、加密、安全、跨平台）。
- **开发中**：遇到不确定点/性能瓶颈/实现复杂度暴涨/要引入新依赖时。
- **重构前**：准备做不兼容重构时（先确认“目标架构”是否已有成熟范式/参考实现）。

## 调研的最小闭环（必须产出）

1. **问题与约束**（写清楚）
   - 目标是什么？可接受的 trade-off 是什么？
   - 约束：平台/性能/内存/网络/安全/许可证/MSRV（最低 Rust 版本）。

2. **候选方案清单**（至少 2 个）
   - 方案 A（首选）/方案 B（备选）/（可选）方案 C（保守兜底）。
   - 每个方案写清：需要引入的依赖、复杂度、风险点、可测试性。

3. **库选型对比**（禁止“只看 star”）
   - `crates.io` / `docs.rs` / GitHub 的维护活跃度、issues、发布频率。
   - API 可用性：是否好用？是否容易 mock/注入？错误类型是否可控？
   - 依赖与 feature：依赖树是否过重？feature 组合是否会膨胀？
   - MSRV / License：是否满足项目约束？

4. **最小 POC（Proof of Concept）**
   - 用最小代码验证关键路径（性能/正确性/易用性）。
   - POC 应该可被删除，不要把“试验代码”混进主实现。

5. **决策记录（ADR）**
   - 记录选择理由与拒绝理由，以及回滚路径。

## 评估清单（可直接复制到 PR）

- [ ] 需求与约束写清（平台、性能、License、MSRV）
- [ ] 至少 2 个候选方案
- [ ] 候选库对比（维护活跃度、API、依赖/feature、MSRV、License）
- [ ] 最小 POC 已验证关键路径
- [ ] ADR 已写（包含备选与回滚）
- [ ] 测试策略已同步（单元/集成/e2e；需要 mock/fake/http-mock 的边界已确定）

## ADR 模板（建议放到 `docs/adr/`）

文件名建议：`docs/adr/0001-<topic>.md`

```text
# ADR-0001: <topic>

## Context
- 我们要解决什么问题？
- 约束是什么（平台/性能/License/MSRV/安全）？

## Decision
- 我们选择什么方案/库？

## Alternatives Considered
- 方案 A：为什么不用？
- 方案 B：为什么不用？

## Consequences
- 正面影响：
- 负面影响/风险：
- 回滚策略：

## Links
- docs.rs / cargo book / perf-book / issues / benchmark 结果等
```

## 常用调研入口（Rust）

- Cargo workspaces / workspace dependencies：用官方文档做准，避免猜语法。
- `crates.io` + `docs.rs`：优先读文档与 API，再看示例与 issue。
- 需要性能优化时：先按 perf-book 做 profiling，再决定是否改算法/数据结构。

## 参考与来源

- Cargo Book:
  Workspaces：https://doc.rust-lang.org/cargo/reference/workspaces.html
- Cargo Reference: Workspace
  dependencies：https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#inheriting-a-dependency-from-a-workspace
- Rust Performance Book：https://nnethercote.github.io/perf-book/
- `just`（命令组织，避免自写杂乱脚本）：https://github.com/casey/just
