# async 测试最佳实践（tokio/async-std）

## 适用范围

当你的被测逻辑使用：

- `async fn`
- `tokio::spawn` / `JoinHandle`
- HTTP/DB/Redis 等异步 I/O

就需要使用 async 测试。

## 推荐：统一使用 tokio（与生产一致）

- 若生产代码使用 `tokio`，测试也使用 `#[tokio::test]`。
- 不要混用多个 runtime（容易出现“在 runtime 外 await/阻塞”的问题）。

## 模式 1：async 单元测试（业务逻辑 + mockall）

当被测对象是纯逻辑，但依赖是 async trait：

- 用 `mockall` mock 依赖
- 用 `#[tokio::test]` 执行

注意：

- 对 `times(n)` 的期望要与真实分支一致（例如 fallback 会多次 query）

## 模式 2：把 async wrapper 降到最小

核心逻辑尽量保持同步/纯函数，异步只做 I/O：

- 纯函数更易测、更稳定
- async 测试只需要覆盖 I/O 交互和 glue code

## 模式 3：避免在测试里用 block_on 嵌套 runtime

- 若已在 `#[tokio::test]` 中，不要再用 `futures::executor::block_on`。
- 统一在 async 环境 `await`。

## 并发相关

- 如果测试涉及并发（spawn 多任务），要用明确的同步点：
  - `join!`/`try_join!`
  - channel
  - barrier

避免：

- 依赖 `sleep` 作为同步手段（不稳定）。

## 与 HTTP mock 结合

- `wiremock` 的 `MockServer::start().await` 要在 async 测试内使用。
- 使用 `expect(n)` 或 `received_requests()` 来断言调用。

## 常见坑

- 使用 `Mutex` 时不要跨 `await` 持锁。
- 避免在测试中混用同步阻塞 API（例如 `std::thread::sleep`）。
