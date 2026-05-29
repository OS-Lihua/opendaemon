# Rust 测试目录结构（通用）

## 目标

- 避免 inline tests（不要在生产 `*.rs` 文件里写
  `#[cfg(test)] mod tests { ... }`）。
- 测试与生产模块结构一致，易于导航与维护。

## 推荐结构（两种主流风格）

Rust 生态里常见两种组织方式：

- **风格 A：`tests/`（标准 integration tests）**
  - 优点：更接近“黑盒”，以 public API 为主，结构是 Rust 社区默认
  - 缺点：较难访问 crate 内部私有 helper（通常需要额外暴露 API 或测试专用入口）

- **风格 B：crate 内测试（白盒）**
  - 典型：`src/tests/`（自定义模块树）
  - 优点：可直接访问内部模块（更适合快速迭代、重构频繁的阶段）
  - 缺点：更容易和实现细节绑定，重构时需要同步调整测试

建议：

- 面向稳定库/对外 API：优先 `tests/`
- 内部快速迭代/需要深度断言：优先 crate 内测试

### 风格 B 示例：crate 内 `src/tests/`

```text
src/
├── lib.rs
├── module_a/
│   ├── mod.rs
│   └── service.rs
└── tests/
    ├── mod.rs
    └── module_a/
        ├── mod.rs
        └── service.rs
```

### 必须的挂载点

- 在 `src/lib.rs`：

```rust
#[cfg(test)]
mod tests;
```

- 在 `src/tests/mod.rs`：

```rust
mod routing;
mod nodes;
```

- 在 `src/tests/routing/mod.rs`：

```rust
mod router;
```

## 何时拆分文件

- 当一个测试文件超过 ~300 行时，建议按场景拆分：
  - `service_errors.rs`
  - `service_sticky.rs`
  - `fallback_behavior.rs`

保持每个文件一个主题，便于定位与演进。

## 常见坑

- **漏了 mod 声明**：目录创建了但 `mod.rs` 未引用，导致测试文件不编译。
- **误解 `tests/` 与 `src/tests/` 的边界**：
  - `tests/` 是 Rust 标准集成测试目录（每个文件是一个独立 crate）。
  - `src/tests/` 是 crate 内部模块（属于同一个 crate，可访问内部模块）。
