# 代码开发与设计原则（KISS / LISP / DRY / SOLID）

> 目标：简洁、可组合、可测试、可重构。避免“复杂度债务”。

## 总原则（强制）

1. **KISS**：能用 1 个简单结构解决，就不要引入 3 个抽象层。
2. **DRY**：避免复制粘贴，但也不要为“未来可能复用”提前抽象。
3. **SOLID**：边界清晰、职责单一、依赖倒置（对 trait 编程）。
4. **LISP（本项目语境）**：偏函数化与组合：
   - 小函数 + 显式参数 + 少副作用
   - 数据驱动（DTO/enum/struct）优先于隐式状态机

## 结构化落地（建议按顺序检查）

### 1) 先分层、再写实现

- domain/app/infra/adapters 的边界只通过 trait/DTO 交互。
- 禁止把 infra 具体类型（DB client、HTTP client）直接塞到 domain/app。

### 2) 依赖注入（DI）只发生在边界

- `app` 只依赖 trait：例如 `trait UserRepo`、`trait Clock`、`trait HttpClient`。
- `adapters` 负责把 `infra` 的实现注入 `app`。
- 这样测试可以用 mock/fake 替换实现（详见 `../../rust-testing/SKILL.md`）。

### 3) 模块与文件拆分（强制）

单文件不要无限增长：

- 600 行：黄牌（必须拆分并给计划）
- 800 行：红牌（必须立即拆分）

用 `references/size-gates.md` 的门禁脚本做自动检查。

### 4) 错误处理与边界

- domain：尽量用业务语义错误（enum），避免 `anyhow` 淹没语义。
- app：负责把 domain 错误映射到 usecase 语义（仍保持可测试）。
- adapters：把 usecase 错误映射成协议错误（HTTP status / gRPC status / CLI exit
  code）。

### 5) 代码评审清单（可直接复制）

- [ ] 边界清楚：domain/app/infra/adapters 的依赖方向正确
- [ ] 新增/变更的公共 API 有对应测试
- [ ] 复杂逻辑有单元测试覆盖边缘条件
- [ ] 外部依赖通过 trait 注入，可 mock/fake
- [ ] 文件/函数体大小受控（门禁脚本不红牌）
- [ ] `cargo fmt` / `cargo test` / `cargo clippy -- -D warnings` 全绿

## 参考与来源

- SOLID/DRY/KISS（概念定义可查）：https://en.wikipedia.org/wiki/SOLID、https://en.wikipedia.org/wiki/Don%27t_repeat_yourself、https://en.wikipedia.org/wiki/KISS_principle
- Cargo
  workspaces（帮助拆分边界）：https://doc.rust-lang.org/cargo/reference/workspaces.html
- 本项目门禁脚本：`../scripts/rust_size_gate.py`
