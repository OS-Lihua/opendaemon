# rust-project-dev / references

本目录用于存放 **rust-project-dev** skill 的“可落地”参考文档（比 `SKILL.md`
更细）。

## 导航

- `research.md`：开发前/开发中调研流程（避免造轮子、库选型清单、ADR 模板）。
- `workspace.md`：workspace 与多 crate
  分层组织（依赖方向、统一配置、依赖治理）。
- `design.md`：KISS / LISP / DRY / SOLID 等编码与设计原则的落地清单。
- `refactor.md`：不兼容重构（强制）：迁移步骤、删除旧代码、门禁策略。
- `size-gates.md`：单文件/函数体行数红黄牌门禁 + `rust_size_gate.py` 使用与 CI
  接入。
- `testing.md`：测试总纲（测试金字塔、test suites、e2e：pytest/behave），并指向
  `rust-testing` 深入细节。
- `e2e-python.md`：e2e 的 pytest/behave（BDD）落地结构与 suites
  切分（与模板对齐）。
- `template-guide.md`：从当前项目初始化新项目的建议流程与注意事项。
- `justfile.md`：用 `justfile` 替代 Makefile
  的项目级命令组织方式（与模板对齐）。

## 本地模板

当前项目为模板：可直接复制/改造的工程模板（含
`justfile`、`e2e/`、`rustfmt.toml`、`prek` 配置等）。
