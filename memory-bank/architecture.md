# 架构说明（Step 2：xq-core 已落地）

正式版代码落地在 `restruct/`，采用 Rust workspace 分层：

## 顶层结构

- Workspace 根：`restruct/Cargo.toml`
- Workspace 根职责：统一工作区依赖与 edition/rust-version
- Workspace members：`members = ["crates/*"]`，所有正式版 crate 统一纳入

## Crates 与职责

- 领域核心（xq-core）：`restruct/crates/xq-core`
- xq-core 入口：`restruct/crates/xq-core/src/lib.rs`
- xq-core 当前状态：已实现 Step 2（数据模型 / FEN / diff / flip / 合法性校验）
- xq-core 未来定位：继续作为规则与坐标系统的唯一权威来源（避免各层重复实现规则）

- 识别管线（xq-vision）：`restruct/crates/xq-vision`
- xq-vision 入口：`restruct/crates/xq-vision/src/lib.rs`
- xq-vision 当前状态：`vision_healthcheck()`，并引用 `xq-core` 验证依赖连通
- xq-vision 未来定位：窗口截图/图片输入/YOLO 推理/后处理

- 引擎适配（xq-engine）：`restruct/crates/xq-engine`
- xq-engine 入口：`restruct/crates/xq-engine/src/lib.rs`
- xq-engine 当前状态：`engine_healthcheck()`，并使用 tracing 打点
- xq-engine 未来定位：UCI/UCCI 适配、进程管理、info/bestmove 解析

- 连线同步（xq-link）：`restruct/crates/xq-link`
- xq-link 入口：`restruct/crates/xq-link/src/lib.rs`
- xq-link 当前状态：`link_healthcheck()`，串起 `xq-vision` 与 `xq-core`
- xq-link 未来定位：窗口绑定、对齐状态机、输入注入与确认

- 应用入口（xq-app）：`restruct/crates/xq-app`
- xq-app 入口：`restruct/crates/xq-app/src/main.rs`
- xq-app 当前状态：CLI 级应用骨架 + 资源自检 + 跨 crate 连通性检查
- 资源自检目标：`libs/large.onnx`
- 资源自检目标：`libs/rotate.onnx`
- 资源自检目标：`libs/pikafish/pikafish.nnue`
- 资源自检目标：`libs/pikafish/<平台二进制>`

## 关键设计点（为后续步骤服务）

- 先打通“workspace 编译链路 + crate 依赖关系 + 资源定位策略”，再进入规则与功能实现（Step 2+）。
- `xq-app` 已承担“资源自检入口”的角色，后续可无缝挂接 iced UI 启动流程与设置页自检按钮。

## xq-core 文件说明（Step 2 新增）

这些文件全部位于 `restruct/crates/xq-core/src/`：

- `lib.rs`：crate 统一入口与对外 re-export；让上层只依赖 `xq_core::*`，不关心内部模块拆分。

- `types.rs`：核心领域模型与坐标系统。
- `types.rs` 主要类型：`Side / PieceKind / Piece / Pos / Move / Board / GameRecord`。
- `types.rs` 关键约定：`Pos` 是 ICCS 逻辑坐标；`Board.grid[row][col]` 的映射为 `row = 9 - rank`、`col = file`。
- `types.rs` 关键能力：`Board::startpos / empty / get / set / find_king / apply_move_unchecked / clone_with_move_unchecked`。

- `fen.rs`：中象 FEN 的解析与规范化生成。
- `fen.rs` 主要入口：`parse_fen`、`Board::from_fen`、`Board::to_fen`。
- `fen.rs` 设计意图：将“FEN 解析策略”和“Board 表示”绑定在 core 内，避免各层自行拼 FEN。

- `diff.rs`：两帧局面的差分分类，服务于识别去抖与同步状态机。
- `diff.rs` 主要入口：`diff_boards`、`BoardDiffKind`、`DiffedMove`。
- `diff.rs` 关键语义：diff 结果是“候选事件”，不是落盘指令；只有 `MoveCandidate` 才可能进入后续确认流程。

- `flip.rs`：棋盘反转与坐标映射（180° 旋转语义）。
- `flip.rs` 主要入口：`flip_pos`、`flip_move`、`map_pos_with_flip`、`Board::flipped`。
- `flip.rs` 关键语义：翻转同时作用于 file 与 rank（等价 180° 旋转），便于统一 UI/逻辑坐标处理。

- `legality.rs`：v1.0 范围内的走法合法性校验。
- `legality.rs` 主要入口：`check_move_legality`、`Board::apply_move_if_legal`、`LegalMoveError`。
- `legality.rs` 覆盖范围：各子移动规则 + 将帅照面约束；不包含长将/长捉/重复局面/60 回合等裁定规则。

## xq-core 架构要点（给后续开发者）

- 规则权威性：走法规则、坐标映射、FEN 规范化应只在 `xq-core` 实现一次。
- 坐标一致性：上层（vision/link/app）尽量在边界处完成坐标转换，进入 core 后统一用 `Pos/Move/Board`。
- diff 的正确用法：`diff_boards` 只能产生候选事件，必须交给上层状态机做确认与回退策略。
- 翻转的正确用法：连线模式下的方向策略应在上层决定是否 `flipped`，但翻转算法本身固定在 core。
