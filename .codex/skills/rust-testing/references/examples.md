# 示例代码（可复制的最小样例）

> 目标：提供少量“最小可运行”的示例，用来指导你在任意 Rust 项目里快速落地测试。
> 这些示例刻意保持短小：强调结构与断言方式，而不是某个具体业务。

## 1) mockall：mock trait + 断言调用参数

**场景**：被测逻辑依赖某个 trait（例如 repository/service），你希望隔离
I/O，并断言传入参数。

```rust
use async_trait::async_trait;
use mockall::mock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: u64,
    pub name: String,
}

#[async_trait]
pub trait UserRepo: Send + Sync {
    async fn get_user(&self, id: u64) -> anyhow::Result<Option<User>>;
}

pub struct UserService<'a> {
    repo: &'a dyn UserRepo,
}

impl<'a> UserService<'a> {
    pub fn new(repo: &'a dyn UserRepo) -> Self {
        Self { repo }
    }

    pub async fn get_username(&self, id: u64) -> anyhow::Result<Option<String>> {
        Ok(self.repo.get_user(id).await?.map(|u| u.name))
    }
}

mock! {
    pub Repo {}

    #[async_trait]
    impl UserRepo for Repo {
        async fn get_user(&self, id: u64) -> anyhow::Result<Option<User>>;
    }
}

#[tokio::test]
async fn get_username_forwards_id_and_maps_result() {
    let mut repo = MockRepo::new();
    repo.expect_get_user()
        .times(1)
        .returning(|id| {
            assert_eq!(id, 42);
            Ok(Some(User { id, name: "alice".to_string() }))
        });

    let svc = UserService::new(&repo);
    let name = svc.get_username(42).await.unwrap();
    assert_eq!(name, Some("alice".to_string()));
}
```

**要点**：

- `.returning(|arg| { assert_eq!(...); Ok(...) })`
  是断言“参数透传”的高性价比写法。
- 不要断言与需求无关的字段（会让测试脆弱）。

## 2) proptest：property-based（不变量）测试

**场景**：你的逻辑对输入空间很大（边界很多），但可以定义清晰不变量。

```rust
use proptest::prelude::*;

fn clamp_0_100(x: i32) -> i32 {
    x.max(0).min(100)
}

proptest! {
  #[test]
  fn clamp_is_always_in_range(x in any::<i32>()) {
    let y = clamp_0_100(x);
    prop_assert!(0 <= y && y <= 100);
  }

  #[test]
  fn clamp_is_idempotent(x in any::<i32>()) {
    let y = clamp_0_100(x);
    prop_assert_eq!(clamp_0_100(y), y);
  }
}
```

**要点**：

- 先写不变量（invariants），再让 proptest 扫描输入空间。
- 失败样本 shrink 后，建议复制成固定 `#[test]` 回归用例。

## 3) wiremock：HTTP mock（推荐）

**场景**：你有 HTTP client（reqwest/ureq 等），但不希望测试依赖真实网络。

```rust
use wiremock::matchers::{method, path, header};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn http_client_sends_token_header() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/ping"))
        .and(header("X-Token", "t123"))
        .respond_with(ResponseTemplate::new(200).set_body_string("pong"))
        .expect(1)
        .mount(&server)
        .await;

    let url = format!("{}/v1/ping", server.uri());
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header("X-Token", "t123")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "pong");
}
```

**要点**：

- base_url 必须可注入（`server.uri()`）。
- `.expect(1)` 用来防止“没打到 mock 也没发现”的假通过。

## 4) mockito：更轻量的 HTTP mock（适合简单场景）

```rust
#[test]
fn mockito_basic_get() {
    let mut server = mockito::Server::new();

    let _m = server
        .mock("GET", "/hello")
        .with_status(200)
        .with_body("world")
        .create();

    let url = format!("{}/hello", server.url());
    let body = ureq::get(&url).call().unwrap().into_string().unwrap();

    assert_eq!(body, "world");
}
```

**要点**：

- mockito 的 async 用法要用 `*_async`（详见 docs.rs），避免在 tokio runtime
  里阻塞。
- 对于更复杂的 matcher/断言，wiremock 通常更合适。
