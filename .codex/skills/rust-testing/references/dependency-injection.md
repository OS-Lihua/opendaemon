# 外部依赖解耦与依赖注入（DI）

## 目标

让 Service / Use Case / 业务逻辑模块在测试中可控：

- 外部依赖（DB/Redis/HTTP）可替换为 mock/fake
- 测试不需要真实基础设施

## 推荐原则：把 I/O 边界建模为 trait

示例：

- `Repository / Dao`：数据访问
- `SessionStore / CacheStore`：会话/状态读写
- `HttpClient`：对外 HTTP

核心逻辑只依赖 trait（`dyn Trait` 或泛型参数），生产环境注入真实实现，测试注入
mock。

## 设计建议

- **trait 要小**：按用例拆分，避免“万能 service”
- **返回值语义清晰**：例如区分 NotFound vs Empty
- **错误类型可映射**：例如 领域错误 -> 对外错误（API/ErrorCode）

## 什么时候用 fake 而不是 mock

- 需要更真实的状态演进（例如 session/state 的写回）
- 需要更高可读性

例如：

- 用 `Mutex<HashMap<...>>` 实现一个 in-memory fake store

## 断言策略

- 单测：断言算法行为（输出、分支命中）
- 集成：断言“输入参数透传/错误映射/对外副作用（写库、写缓存、发请求）”

## 与测试目录结构结合

建议为 fake/mock 工具提供复用模块（放在 `tests/` 或 crate 内测试目录均可）：

- `tests/support/mod.rs` 或 `src/tests/support/mod.rs`
- `tests/fakes/*.rs` 或 `src/tests/fakes/*.rs`

避免每个测试文件重复造轮子。
