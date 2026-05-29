# 使用 `template/` 从零组织项目

> 目标：从一开始就把“门禁/结构/测试/脚手架”铺好，减少后期补洞成本。

## 模板

模板包含（概览）：

- `justfile`：项目命令入口（含 `init/test/e2e/happy`）
- `rustfmt.toml`：统一格式化
- `typos.toml` + `.autocorrect*`：拼写/自动修正配置（配合 `prek`）
- `e2e/`：Python e2e 工程（`uv` + `pytest`，并预留 `behave`）
- `src/tests/`：集中测试目录示例

## 从模板启动新项目（建议步骤）

1. **复制模板到新仓库**
   - 把 `template/` 复制到新目录（作为项目根）。
   - 注意：模板内包含 `.git/`（用于模板自身演示），新项目应删除并重新
     `git init`。

2. **改名与元信息**
   - 修改 `Cargo.toml` 的 package/workspace
     元信息（name、license、rust-version、edition）。
   - 补齐 `README.md` 与 `docs/` 的架构/模块设计说明。

3. **确定 workspace + crates 分层**
   - 先画出 domain/app/infra/adapters，再决定 crates 拆分。
   - 依赖统一放到 `[workspace.dependencies]`，减少漂移（见
     `references/workspace.md`）。

4. **建立门禁**
   - `just lint` / `just test` / `just e2e`（模板已有）
   - 将 `references/size-gates.md` 的行数门禁接入 justfile/CI

5. **E2E 工程化**
   - `e2e/pyproject.toml` 里的 Python 版本/依赖按团队环境调整
   - 按 `references/testing.md` 先定义 test
     suites（unit/integration/e2e），再写用例

## 参考与来源

- 模板 `justfile`：`template/justfile`
- 模板 e2e：`template/e2e/README.md`
- workspace 组织：`references/workspace.md`
- 行数门禁：`references/size-gates.md`
