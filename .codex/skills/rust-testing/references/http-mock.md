# HTTP Mock（wiremock / mockito）最佳实践

> 通用原则：只有在“确实存在 HTTP 依赖”的模块中才引入 HTTP
> mock。不要为了测试而引入真实网络。

## 选型

- 推荐优先使用 **wiremock**：
  - matcher 丰富（method/path/header/query/body_json 等）
  - 支持 request recording（便于断言请求体）
  - 更适配 async 测试（tokio）

- **mockito** 更适合简单场景：
  - 快速 mock 少量 endpoint
  - 断言维度相对简单

## wiremock 推荐套路

### 1) 可注入 base_url

生产代码里不要 hardcode base url，构造 client 时传入：

- `Client::new(base_url, token)`

这样测试可以把 base_url 指向 `MockServer::uri()`。

### 2) 使用 expect 限定调用次数

- `.expect(1)`：确保调用次数符合预期
- `.named("...")`：当期望未满足时更容易定位

API 参考（来源：docs.rs）：

- https://docs.rs/wiremock/latest/wiremock/struct.Mock.html
- https://docs.rs/wiremock/latest/wiremock/struct.MockServer.html

### 3) 断言请求体

两种方式：

- **方式 A（优先）**：使用 matcher（例如 `body_partial_json`）
- **方式 B**：使用 `mock_server.received_requests()` 读取请求，再对 body 做 JSON
  断言

最小示例见：

- `references/examples.md`（wiremock 示例）

`received_requests` API 参考（来源：docs.rs）：

- https://docs.rs/wiremock/latest/wiremock/struct.MockServer.html

## 避免测试不稳定

- 不要依赖真实网络
- 不要依赖真实时间（必要时注入 clock）
- 不要把随机种子隐藏在 HTTP mock 行为中

## mockito 注意事项

- mockito 同时提供 sync 与 async API。
- 如果你的测试运行在 tokio runtime（例如 `#[tokio::test]`），避免在 runtime
  内调用会阻塞线程的 sync 客户端；优先使用 mockito 的 async
  接口（`*_async`）或改用 wiremock。
- 最小示例见：`references/examples.md`（mockito 示例）
