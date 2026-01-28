# 进度记录（2026-01-27）

## 已完成：实施计划 Step 1（项目结构与基础工程）

完成内容：
- 新建正式版根目录与 Rust workspace：`restruct/Cargo.toml`
- 新建五个 crate 骨架（均为 edition 2024）：
  - `restruct/crates/xq-core`
  - `restruct/crates/xq-vision`
  - `restruct/crates/xq-engine`
  - `restruct/crates/xq-link`
  - `restruct/crates/xq-app`
- 为所有 crate 预留 `rotate` / `gpu` feature（当前为空实现，用于先打通构建链路）
- 在 `xq-app` 启动流程加入资源自检（开发态从 `CARGO_MANIFEST_DIR` 反推仓库根目录，并检查 `libs/` 下模型与 Pikafish 资源）

关键文件入口：
- Workspace 根：`restruct/Cargo.toml`
- 应用入口：`restruct/crates/xq-app/src/main.rs`

验证说明：
- 受网络限制，在线拉取依赖失败；已使用离线模式验证基础构建与测试：
  - `cargo test -q --offline --manifest-path restruct/Cargo.toml`
  - `cargo build -q --offline --manifest-path restruct/Cargo.toml --features rotate`
  - `cargo build -q --offline --manifest-path restruct/Cargo.toml --features gpu`
- 用户已确认测试通过（“confirmed”）。

边界约束：
- 按用户要求，尚未进入 Step 2（`xq-core` 规则与数据模型实现）。

---

## 已完成：实施计划 Step 2（xq-core 规则与数据模型）

完成内容（均落地在 `restruct/crates/xq-core/src/`）：
- 核心数据模型（2.1）：
  - 新增 `types.rs`：`Side / PieceKind / Piece / Pos / Move / Board / GameRecord`
  - 提供标准开局常量：`STARTPOS_FEN`
  - 提供不校验合法性的落子与克隆落子能力：`apply_move_unchecked / clone_with_move_unchecked`
- FEN 解析与生成（2.2）：
  - 新增 `fen.rs`：`parse_fen`、`Board::from_fen`、`Board::to_fen`
  - 提供 `FenParseOptions` 与 `FenError`
- 局面差分（2.3）：
  - 新增 `diff.rs`：`diff_boards`、`BoardDiffKind`、`DiffedMove`
  - diff 仅产出“候选事件”，不直接落盘
- 棋盘翻转与坐标映射（2.4）：
  - 新增 `flip.rs`：`flip_pos / flip_move / map_pos_with_flip`
  - 提供 `Board::flipped()`（180° 翻转）
- 走法合法性校验（2.5，v1.0 范围）：
  - 新增 `legality.rs`：`check_move_legality`、`LegalMoveError`
  - 覆盖各子移动规则与“将帅照面”基础约束

对外导出（`restruct/crates/xq-core/src/lib.rs`）：
- re-export 了 Step 2 相关 API，供后续 crate 直接引用

给后续开发者的关键信息（非常建议先读）：
- 坐标系约定是“ICCS 逻辑坐标为主、数组索引为实现细节”：
- `Pos { file, rank }` 以红方视角：`file=0..8` 对应 `a..i`，`rank=0..9` 对应 `0..9`（`0` 是红方底线）。
- 内部数组 `grid[row][col]` 采用“上方是高位 rank 9”的布局：
- 映射规则固定为：`row = 9 - rank`，`col = file`（见 `Pos::to_index / Pos::from_index`）。
- `Board::side_to_move` 是事实来源（source of truth）：
- 合法性校验会强制“起点棋子阵营 == side_to_move”（见 `check_move_legality`）。
- `diff_boards` 的结果是“候选事件”，不是落盘指令：
- 只有 `BoardDiffKind::MoveCandidate` 且通过上层确认窗口/状态机校验后，才应该真正落子。
- 合法性覆盖范围遵循 PRD 边界：
- 当前只实现“棋子移动规则 + 将帅照面约束”，没有实现长将/长捉/重复局面/60 回合等裁定规则。
- 棋盘翻转是 180° 旋转，而非仅镜像：
- `flip_pos / flip_move / Board::flipped` 都是“file 与 rank 同时反转”的等价变换。

验证说明（离线模式）：
- 仅跑 Step 2 相关 crate：
  - `cargo test --offline -p xq-core --manifest-path restruct/Cargo.toml`
- 跑整个 workspace：
  - `cargo test -q --offline --manifest-path restruct/Cargo.toml`
- 结果：全部测试通过（xq-core 29/29）。

边界约束：
- 按用户要求：到此为止，不进入 Step 3（xq-engine）。

---

## 已完成：实施计划 Step 3（xq-engine 引擎抽象与 UCI/UCCI 适配）

完成内容（均落地在 `restruct/crates/xq-engine/src/`）：
- 引擎抽象接口与事件模型（3.1）：
  - 新增 `adapter.rs`：`EngineAdapter` trait、`EngineEvent / EngineInfo / EngineBestMove / EngineScore`
  - 提供工厂函数：`create_engine(profile)`（按协议创建适配器）
- 协议与参数系统（3.1 / 3.4）：
  - 新增 `protocol.rs`：`EngineProtocol::{Uci,Ucci}`
  - 新增 `profile.rs`：`EngineProfile / EngineOptions / EngineOption / SearchParams`
  - 统一 `go` 指令映射：UCI 使用 `movetime`，UCCI 使用 `time`
  - 统一 `setoption` 生成入口：`EngineProfile::to_setoption_commands`
- 输出解析（3.1 基础能力）：
  - 新增 `parser.rs`：`parse_engine_line`
  - 结构化解析 `info`（depth/score/pv 等）与 `bestmove`
  - PV 走法按 ICCS 解析为 `xq_core::Move`，同时保留 `pv_raw`
- 进程管理与握手（3.1 基础设施）：
  - 新增 `process.rs`：`EngineProcess`
  - 使用 `std::process::Command` + 后台读取线程 + channel 回传事件
  - 提供握手与就绪检查：`handshake()` / `ensure_ready()`
- 协议适配器（3.2 / 3.3）：
  - 新增 `uci.rs`：`UciAdapter`
  - 新增 `ucci.rs`：`UcciAdapter`
  - `apply_profile()` 走标准流程：握手 → setoption → isready
- crate 入口导出更新：
  - 更新 `restruct/crates/xq-engine/src/lib.rs`：按模块拆分并 re-export 关键类型

给后续开发者的关键信息（强烈建议先读）：
- 当前 API 是“同步接口 + 事件拉取模型”：
  - 上层通过 `EngineAdapter` 发命令（init/position/go/stop/quit）
  - 通过 `try_recv_event` 或 `recv_event_timeout` 拉取事件
- 解析器的坐标假设是 ICCS：
  - `parser.rs` 会尝试用 `Move::from_iccs` 解析 PV 与 bestmove
  - 非 ICCS 的引擎输出会退化为仅保留 raw 文本
- Profile 是协议映射的唯一入口（避免上层散落协议分支）：
  - `SearchParams::to_go_command(protocol)`
  - `EngineProfile::to_setoption_commands()`
- 握手阶段会“消费输出流事件”：
  - `EngineProcess::handshake / ensure_ready` 会从 channel 读取直到命中目标行
  - 上层不要假设握手前后的所有 raw 行都可见

验证说明（离线模式）：
- 仅跑 Step 3 相关 crate：
  - `cargo test --offline -p xq-engine --manifest-path restruct/Cargo.toml`
  - 结果：测试通过（xq-engine 8/8）
- 跑整个 workspace：
  - `cargo test --offline --manifest-path restruct/Cargo.toml`
  - 结果：全部测试通过

测试设计说明（重要）：
- `restruct/crates/xq-engine/src/uci.rs` 内含一个“假 UCI 引擎”测试：
  - 测试会在临时目录生成一个可执行 shell 脚本，模拟 uci/isready/go/bestmove
  - 该测试用于稳定验证：握手流程、输出解析、事件通道链路

边界约束：
- 当时按用户要求：到此为止，不进入 Step 4（xq-vision）；后续已在下文完成 Step 4。

---

## 已完成：实施计划 Step 4（xq-vision 识别管线）

完成内容（均落地在 `restruct/crates/xq-vision/src/`）：
- 输入源统一抽象（4.1）：
  - 新增 `input.rs`：`CaptureInput / WindowCapture / ImageFile / Frame / FrameSource`
  - 同一识别管线可复用窗口截图与图片文件输入
- 模型加载与推理（4.2）：
  - 新增 `model.rs`：`ModelPaths / VisionModel`，支持 large / rotate 模型文件
  - 新增 `detect.rs`：YOLO 推理与 NMS、`LABELS / LIMIT` 常量与 `ModelKind`
  - 兼容 macOS CoreML EP，并显式钉住 `ort-sys = 2.0.0-rc.9`
- 裁剪锁定与区域管理（4.3）：
  - 新增 `crop.rs`：`CropRegion / CropLock` 与 `board_crop_region`（支持半格 padding）
  - 支持首次定位棋盘框后锁定裁剪区域以降低推理成本
- detections → Board 后处理（4.4）：
  - 新增 `postprocess.rs`：`detections_to_grid / detections_to_observation`
  - 产出 `BoardObservation`（camp / flipped / board / raw_grid）
  - 黑方底线视角时使用 `xq_core::flip_pos` 做 180° 翻转
- 识别稳定性机制（4.5）：
  - 新增 `stability.rs`：`StabilityFilter` 基于 `diff_boards` 做候选事件过滤
  - `OneChanged` 计数超阈值触发 reset
- 管线编排：
  - 新增 `pipeline.rs`：`VisionPipeline / PipelineConfig / PipelineOutput`
  - 支持单帧分析与二次确认（confirm delay）

依赖调整：
- Workspace 依赖新增 `ndarray` / `xcap`（`restruct/Cargo.toml`）。
- `xq-vision` 增加 `ort` 与平台特性依赖，并显式钉住 `ort-sys = 2.0.0-rc.9` 以避免 rc.10 兼容性问题。

验证说明（离线模式）：
- 仅跑 Step 4 相关 crate：
  - `cargo test --offline -p xq-vision --manifest-path restruct/Cargo.toml`
  - 结果：测试通过（xq-vision 13/13）
- 跑整个 workspace：
  - `cargo test --offline --manifest-path restruct/Cargo.toml`
  - 结果：全部测试通过

边界约束：
- 按用户要求：到此为止，不进入 Step 5（xq-link）。

---

## 已完成：实施计划 Step 5（xq-link 连线与双向同步骨架）

完成内容（均落地在 `restruct/crates/xq-link/src/`）：
- 窗口枚举与连接（5.1）：
  - 新增 `window.rs`：`list_windows / LinkWindow / LinkWindowInfo / WindowPosition`
  - 基于 `xcap` 枚举窗口、定位目标窗口并输出基础信息
- 对齐机制与同步状态机（5.2）：
  - 新增 `sync.rs`：`SyncState / ExternalUpdate / PendingInjection`
  - 覆盖 AwaitingExternal / Aligned / Desynced 等状态流转与候选走法确认
- 输入注入（5.3）：
  - 新增 `inject.rs`：`InputInjector` trait + `EnigoInjector`
  - 以“点击起点格 → 点击终点格”的最小可靠方案作为注入基线
- 注入后确认（5.4）：
  - 新增 `runtime.rs`：`LinkRuntime`（注入后可触发二次识别确认）
  - 统一封装“截图识别 → 对齐更新 → 注入 → 再识别确认”的最小链路
- macOS 权限引导（5.5）：
  - 新增 `permission.rs`：`check_input_permission`（检测辅助功能/输入监控授权）
- 对外导出整理（`lib.rs`）：
  - re-export `window / geometry / sync / inject / runtime / permission` 核心类型

关键文件入口：
- `restruct/crates/xq-link/src/window.rs`
- `restruct/crates/xq-link/src/geometry.rs`
- `restruct/crates/xq-link/src/sync.rs`
- `restruct/crates/xq-link/src/inject.rs`
- `restruct/crates/xq-link/src/runtime.rs`
- `restruct/crates/xq-link/src/permission.rs`

依赖调整：
- Workspace 新增 `enigo`（`restruct/Cargo.toml`）
- `xq-link` 引入 `xcap / enigo`（`restruct/crates/xq-link/Cargo.toml`）

验证说明（离线模式）：
- 仅跑 Step 5 相关 crate：
  - `cargo test --offline -p xq-link --manifest-path restruct/Cargo.toml`
  - 结果：测试通过（xq-link 9/9）
- 用户已确认测试通过（“Confirmed”）。

边界约束：
- 按用户要求：到此为止，不进入 Step 6（xq-app）。

---

## 已完成：实施计划 Step 6（xq-app iced UI + 模式系统最小可用）

完成内容（主要落地在 `restruct/crates/xq-app/src/`）：
- iced UI 主布局（6.1）：
  - 顶部工具栏 + 棋盘区域 + 右侧侧栏（分析/棋谱/提示）；
  - 模式切换、学习提示开关、侧栏折叠。
- 棋盘组件（6.2）：
  - 改为 `canvas` 绘制，棋子落在**交叉点**（而非格子内）；
  - 绘制棋盘线、河界断线、九宫斜线；
  - 选中/推荐走法高亮、连线模式自动同步方向（识别视角翻转）。
- 模式与生命周期（6.3）：
  - 模式切换时清理状态；连线模式退出即停止连线；
  - 人机对弈/AI vs AI 的回合控制逻辑已接通。
- 引擎与云库串联（对齐 Step 3/3.5 规划）：
  - 侧栏“分析”区集成引擎/云库开关；
  - 云库（chessdb）优先命中，失败自动回退本地引擎；
  - 引擎用于提示、AI 落子、连线分析的最小可用结果。
- 连线入口（对齐 Step 5/6 规划）：
  - 侧栏提供“窗口列表选择/刷新/开始/停止连线”；
  - 连线启动后周期识别并更新棋盘；
  - 连线对战模式下支持本地走子 → 注入外部窗口 → 确认回写；
  - macOS 输入权限检测与提示已接入。

新增/调整文件：
- `restruct/crates/xq-app/src/app.rs`：UI、棋盘 canvas、引擎/云库/连线逻辑汇总。
- `restruct/crates/xq-app/src/resources.rs`：资源路径与对外访问。
- `restruct/crates/xq-engine/src/chessdb.rs`：云库查询实现（无额外依赖，基于 TcpStream）。
- `restruct/crates/xq-engine/src/lib.rs`：导出 chessdb 接口。
- `restruct/Cargo.toml`：`iced` 启用 `canvas` 特性。

验证说明（离线模式）：
- `cargo check --offline -p xq-app --manifest-path restruct/Cargo.toml`
- 用户已确认测试通过（“Confirmed”）。

边界约束：
- 按用户要求：到此为止，不进入 Step 7（棋谱与局势曲线）。另：引擎/连线为最小可用串联，后续需继续完善 UI 与策略细节。
