# 常见坑与排查清单

## 1) mockall closure move/clone

症状：

- 变量被 move 进 `returning(move |...| ...)` 后，后面还要用导致编译错误

解决：

- `let expected = value.clone();`
- closure 中用 `expected.clone()`

## 2) 过度断言导致测试脆弱

症状：

- 轻微重构就大量测试失败

解决：

- 只断言“业务关键字段”
- 不做全字段结构比较

## 3) 随机选择导致 flaky

症状：

- 同一测试偶发失败

解决：

- 让候选集只剩唯一有效节点
- 或抽象 RNG 允许注入种子

## 4) async + Mutex 跨 await

症状：

- 死锁/性能异常

解决：

- 锁只在同步段持有，不要跨 await

## 5) rust-analyzer proc-macro ABI mismatch

症状：

- IDE 报错：mismatched ABI（mockall/async_trait/tokio_macros）

说明：

- 通常是 rust-analyzer 缓存/工具链不一致
- 不等于 `cargo test` 会失败

解决：

- Restart Rust Analyzer
- 必要时清理 target（会删缓存，谨慎）

## 6) 测试模块未挂载

症状：

- 写了 `src/tests/foo.rs` 但没有被编译

解决：

- `lib.rs` 加 `#[cfg(test)] mod tests;`
- `src/tests/mod.rs` 引用子模块

## 7) 混用 runtime

症状：

- “cannot start a runtime from within a runtime”

解决：

- 在 `#[tokio::test]` 中不要再 `block_on`
- 统一 tokio runtime
