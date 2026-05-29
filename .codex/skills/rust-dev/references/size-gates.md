# 文件行数红黄牌 / 函数体行数门禁

> 目标：强制拆分模块与职责，避免“巨型文件/巨型函数”导致的可维护性崩塌。

## 红黄牌规则（强制）

- **黄牌**：单文件 `> 600` 行（必须拆分，并给出拆分计划）
- **红牌**：单文件 `> 800` 行（禁止存在，必须立即拆分）

可选（建议）：

- 函数体黄牌/红牌阈值：用于快速定位“巨型函数”（统计是近似值）。

## 工具：`rust_size_gate.py`

脚本位置（本 skill 内）：

- `../scripts/rust_size_gate.py`

典型用法：

```bash
python3 ../scripts/rust_size_gate.py \
  --root . \
  --glob 'crates/**/*.rs' \
  --warn-file-lines 600 \
  --max-file-lines 800
```

排除生成代码/第三方镜像代码（示例）：

```bash
python3 ../scripts/rust_size_gate.py \
  --root . \
  --glob 'crates/**/*.rs' \
  --exclude-glob '**/generated/**' \
  --exclude-glob 'crates/quinn/**'
```

把黄牌也当失败（CI 更严格时）：

```bash
python3 ../scripts/rust_size_gate.py \
  --root . \
  --glob 'crates/**/*.rs' \
  --warn-file-lines 600 \
  --max-file-lines 800 \
  --fail-on-warn
```

函数体门禁（可选，近似统计）：

```bash
python3 ../scripts/rust_size_gate.py \
  --root . \
  --glob 'crates/**/*.rs' \
  --warn-fn-lines 80 \
  --max-fn-lines 150
```

## 拆分策略（建议）

当文件接近/超过黄牌时，优先按“职责边界”拆：

- 类型与协议：`types.rs` / `dto.rs`
- 纯逻辑：`logic.rs` / `rules.rs`
- I/O 与适配：`io.rs` / `repo.rs` / `client.rs`
- 平台差异：`platform/{linux,macos,windows}/...`

当函数接近/超过阈值时：

- 先把“数据准备/校验/转换/副作用”拆成小函数
- 把分支逻辑拆成 `enum` + 独立 handler（降低圈复杂度）
- 对外部依赖调用点做 trait 封装，便于 mock 与测试

## CI / justfile 接入（建议）

在 `justfile` 中加一个 recipe：

```text
size-gate:
    python3 scripts/rust_size_gate.py --root . --glob 'crates/**/*.rs' --warn-file-lines 600 --max-file-lines 800
```

## 参考与来源

- 本项目脚本：`../scripts/rust_size_gate.py`
- `justfile` 组织方式：`references/justfile.md`
