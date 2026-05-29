# mockall 最佳实践（trait mock）

## 适用范围

- 你需要隔离外部依赖（DB/Redis/HTTP/系统调用）时。
- 你需要断言某个依赖被调用的次数、顺序、参数或返回值时。

典型：

- Repository / DAO
- Session store
- HTTP client wrapper

## 推荐模式：只 mock 边界（I/O）

- 被测对象：Router/Controller/Service 的业务逻辑。
- mock 的对象：其依赖（trait）。

不要 mock 被测对象内部函数（那会把测试变成“复述实现”）。

## async trait 的 mock

当 trait 是 `async fn` 时：

- 使用 `mockall::mock!` 生成 mock
- 若 trait 依赖 `async_trait`，mock 定义里也要标注 `#[async_trait]`

示例结构（示意）：

```rust
mock! {
  pub Repo {}

  #[async_trait]
  impl UserRepo for Repo {
    async fn get_user(&self, id: u64) -> anyhow::Result<Option<User>>;
  }
}
```

## 断言输入参数（推荐）

在 `.returning(move |q| { ... })` 里断言关键字段：

- 只断言与需求直接相关的字段
- 避免把整个 `NodeQuery` 全字段比较（容易因无关改动导致脆弱）

常见断言：

- `q.provider_ids`
- `q.country`（验证 allow_countries fallback / none-country 查询）
- `q.node_type`

## 处理闭包 move/clone 的坑

在 returning closure 里捕获变量时，优先：

- `let expected = provider_ids.clone();`
- 在 closure 中使用 `expected.clone()`

避免把 `provider_ids` move 进去后，后面还要用。

## times / expect 的使用建议

- `.times(0)`：确保不发生额外 I/O（例如 pinned_exit 时不应查询 last hop）
- `.times(1)`：最常见，用于确认分支命中
- `.times(n)`：fallback 需要多次查询时使用（例如 allow_countries 有 2 个国家）

## 何时不要用 mockall

- 纯函数逻辑：直接写单元测试即可。
- 只需要简单 fake：可以写一个小的 in-memory fake struct（更可读、更稳定）。
