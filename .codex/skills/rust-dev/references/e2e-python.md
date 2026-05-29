# E2E（Python）：pytest / behave（BDD）与 Test Suites

> 目标：用 Python 做 e2e 的“外层测试壳”，覆盖关键链路；并通过 suites
> 控制速度与稳定性。

## 1) 选择 pytest 还是 behave？

优先用 **pytest**：

- 工程化断言更强（fixture、参数化、marker、插件）
- 更适合“接口/输出正确性”的回归测试

适合用 **behave（BDD）** 的典型场景：

- 业务更像“角色 - 场景 - 步骤”的叙事（跨模块链路、需要可读性）
- 需要把自然语言需求与用例强绑定（feature 文件可直接评审）

结论：可以两者并存，但**e2e 用例总量必须克制**，只保留关键链路。

## 2) 推荐目录结构（与模板对齐）

模板位置：`template/e2e/`

建议在项目中使用类似结构：

```text
e2e/
  README.md
  pyproject.toml
  data/
    input/
    baseline/
  tests/
    test_*.py              # pytest
    bdd/
      features/
        *.feature
        steps/
          *.py
```

## 3) 用 `uv` 管理 e2e 环境（推荐）

```bash
cd e2e
uv venv .venv
uv sync

# pytest
uv run -- pytest -v --tb=short

# behave（BDD）
uv run -- behave tests/bdd/features
```

> 模板的 `pyproject.toml` 里 Python 版本是 `>=3.14`，请按团队实际环境调整。

## 4) Test suites（强制：拆分“快/慢/不稳定”）

### pytest：用 marker 做 suites

约定几个 suite（示例）：

- `smoke`：极少、极快（PR 必跑）
- `regression`：回归（PR 必跑或主干必跑）
- `slow`：慢测试（按需或 nightly）
- `flaky`：不稳定测试（必须逐步清零，短期可隔离）

运行子集：

```bash
pytest -m smoke -v
pytest -m 'not slow and not flaky' -v
```

### behave：用 tag 做 suites

在 feature/scenario 上标注：

- `@smoke` / `@regression` / `@wip` / `@slow`

运行子集：

```bash
behave -t @smoke tests/bdd/features
behave -t ~@wip tests/bdd/features
```

## 5) e2e 与 Rust 项目衔接（建议）

- e2e 应该尽量 **调用 release 构建产物**（或稳定的 dev
  构建产物），避免每个用例都 `cargo build`。
- 通过环境变量注入可执行文件路径（例如 `APP_BIN=...`），pytest/behave 统一读取。
- 外部依赖（HTTP/DB）尽量 mock 或使用可复现的本地容器，并把启动/清理封装成
  fixtures/hooks。

## 参考与来源

- 模板 e2e 说明：`template/e2e/README.md`
- behave 官方文档：https://behave.readthedocs.io/en/stable/
- pytest 官方文档：https://docs.pytest.org/en/stable/
- `uv` 官方文档：https://docs.astral.sh/uv/
