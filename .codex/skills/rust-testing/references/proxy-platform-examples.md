# proxy-platform：与选路相关的测试落地点（示例导航）

## path-curator：Router 单元测试

- 目录：`crates/path-curator/src/tests/`
- 文件：`crates/path-curator/src/tests/router.rs`

建议覆盖：

- `pinned_exit`：最后一跳固定，且不应再调用 `query_nodes`
- `allow_countries` fallback：按顺序尝试国家
- `allow_countries` 为空：应使用 `country=None` 查询
- 0 权重节点：应被视为不可用，必要时触发 fallback
- 空 hops：应返回 `NoAvailableNode`

## control-plane：RoutingController 集成测试

- 目录：`crates/control-plane/src/tests/routing/`
- 文件：`crates/control-plane/src/tests/routing/routing_controller.rs`

建议覆盖：

- `RouterError -> RoutingError` 错误映射
- sticky session：读取/写回 sticky_exit
- `provider_ids` 透传到 `NodeQuery.provider_ids`
- allow_countries fallback 是否通过委托 Router 生效

## 目录结构入口

- `crates/control-plane/src/lib.rs`：`#[cfg(test)] mod tests;`
- `crates/control-plane/src/tests/mod.rs`：聚合测试子模块
