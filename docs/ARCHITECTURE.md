# 中国象棋学习助手 — 技术架构文档

## 目录

- [项目概述](#项目概述)
- [整体架构](#整体架构)
- [技术栈](#技术栈)
- [模块详解](#模块详解)
  - [前端模块](#前端模块)
  - [后端模块](#后端模块)
- [数据流](#数据流)
- [核心算法](#核心算法)
- [构建与运行](#构建与运行)
- [配置说明](#配置说明)
- [平台差异](#平台差异)
- [扩展开发指南](#扩展开发指南)

---

## 项目概述

**中国象棋学习助手 (xqlink)** 是一款基于 Tauri 2 的跨平台桌面应用。它通过实时截取屏幕中的象棋对局窗口，利用 YOLOv8 深度学习模型识别棋盘局面，再结合 Pikafish 引擎和云端开局库进行分析，为用户提供最佳走法建议。

### 工作流程简述

```
用户选择目标窗口 → 定时截屏 → YOLOv8 棋子检测 → 局面构建
    → 云库查询 / Pikafish 引擎分析 → 中文走法转换 → 前端展示
```

---

## 整体架构

```
┌────────────────────────────────────────────────────────┐
│                    Tauri 桌面应用                        │
├────────────────────┬───────────────────────────────────┤
│     前端 (WebView)  │          后端 (Rust)              │
│                    │                                   │
│  ┌──────────────┐  │  ┌─────────┐  ┌───────────────┐  │
│  │ Toolbar.vue  │  │  │ listen  │  │    worker     │  │
│  ├──────────────┤  │  │ (截屏)   │→│  (状态机循环)  │  │
│  │Chessboard.vue│←─┼──┤         │  └───────┬───────┘  │
│  ├──────────────┤  │  └─────────┘          │          │
│  │ Analyse.vue  │←─┼──────────────────────┐│          │
│  └──────────────┘  │                      ││          │
│                    │  ┌─────────┐  ┌──────▼▼───────┐  │
│                    │  │  yolo   │  │    engine     │  │
│                    │  │(ONNX推理)│  │ (Pikafish +   │  │
│                    │  └─────────┘  │  chessdb云库)  │  │
│                    │               └───────────────┘  │
│                    │  ┌─────────┐  ┌──────────────┐   │
│                    │  │  chess  │  │    config    │   │
│                    │  │(棋局逻辑)│  │  (配置管理)   │   │
│                    │  └─────────┘  └──────────────┘   │
└────────────────────┴───────────────────────────────────┘
```

---

## 技术栈

| 组件 | 技术 | 版本 | 说明 |
|------|------|------|------|
| 桌面框架 | Tauri | 2.5.1 | 跨平台桌面打包 |
| 前端框架 | Vue | 3.5+ | 组合式 API |
| 前端语言 | TypeScript | 5.6 | 类型安全 |
| UI 组件库 | Naive UI | 2.41+ | Vue 3 组件库 |
| 构建工具 | Vite | 6.2+ | 前端打包 |
| 后端语言 | Rust | 2024 edition | 内存安全 |
| AI 推理 | ONNX Runtime | 2.0.0-rc.9 | 跨平台推理 |
| AI 模型 | YOLOv8 | - | 目标检测 |
| 棋力引擎 | Pikafish | - | UCI 协议引擎 |
| 窗口截屏 | xcap | 0.5.1 | 跨平台截屏 |
| HTTP 客户端 | reqwest | 0.12 | 云库查询 |
| 异步运行时 | Tokio | 1.44 | 异步支持 |
| 数值计算 | ndarray | 0.16 | 张量操作 |
| 包管理 | pnpm | - | 前端依赖管理 |

---

## 模块详解

### 前端模块

#### `src/App.vue` — 应用根组件

布局结构：
- 顶部：工具栏 (Toolbar)
- 左下：棋盘 (Chessboard)
- 右下：分析面板 (Analyse)
- 底部：页脚

#### `src/components/Toolbar.vue` — 工具栏

功能：
- **模式选择**：连线分析（已实现）、连线对战（开发中）、人机对弈（开发中）
- **引擎启停**：选择目标窗口 → 启动监听 / 停止监听
- **引擎配置抽屉**：搜索深度、限时、线程数、哈希表大小、云库开关

关键交互：
- `startListen()` — 调用 `list_windows` 获取窗口列表，弹窗让用户选择，然后调用 `start_listen`
- `stopListen()` — 调用 `stop_listen` 停止后台线程
- 配置项通过 `invoke` 实时写入后端并持久化

#### `src/components/Chessboard.vue` — 棋盘渲染

功能：
- 渲染 9×10 的中国象棋棋盘网格
- 监听后端 `position` 事件更新棋子位置
- 监听 `move` 事件动画移动棋子
- 监听 `mirror` 事件切换红方/黑方视角
- 支持高亮最佳走法位置 (`b-select` CSS 类)

实现方式：通过 DOM 操作 (`document.getElementById`) 直接控制棋子 CSS 类名。

#### `src/components/Analyse.vue` — 分析面板

功能：
- 展示最佳走法（中文）、评分、搜索深度
- 滚动日志展示历史分析记录
- 高亮推荐走法的起止位置

---

### 后端模块

#### `server/src/lib.rs` — 应用入口

职责：
- 初始化 Tauri 应用
- 创建全局共享状态 `SharedState`（配置、引擎、监听线程）
- 注册所有 Tauri command 处理器
- 加载配置文件和 Pikafish 引擎

全局状态结构：
```rust
struct SharedState {
    config: Arc<RwLock<Config>>,        // 配置（读多写少）
    engine: Arc<Mutex<Engine>>,          // 引擎（互斥访问）
    listen_thread: Mutex<Option<JoinHandle<()>>>,  // 监听线程句柄
}
```

#### `server/src/listen.rs` — 窗口捕获

职责：
- 枚举系统所有窗口 (`list_windows`)
- 创建 `ListenWindow` 包装指定窗口
- 提供 `capture()` 方法截取窗口图像
- 支持设置子区域裁剪 (`set_sub_bound`)，只截取棋盘区域

关键实现：
- 首次截图后通过 `detections_bound()` 计算棋盘边界
- 后续只截取棋盘子区域，减少推理输入噪声

#### `server/src/yolo.rs` — YOLOv8 推理

职责：
- 加载 ONNX 模型（编译时通过 `include_bytes!` 嵌入）
- 图像预处理（缩放到 640×640，归一化）
- 执行推理，解析输出张量
- 非极大值抑制 (NMS) 去除重叠检测框

棋子分类标签（15类）：
```
小写 = 黑方: n(马) b(象) a(士) k(将) r(车) c(炮) p(卒)
大写 = 红方: R(车) N(马) A(仕) K(帅) B(相) C(炮) P(兵)
'0' = 空位标记
```

每类棋子数量上限：
```
马2 象2 士2 将1 车2 炮2 兵5 (红黑各一套) + 空位1
```

关键参数：
- 输入尺寸: 640 × 640
- 置信度阈值: 0.7
- IOU 阈值: 0.5

GPU 加速策略：
- macOS: CoreML
- Windows (GPU feature): CUDA + DirectML
- Windows (CPU): 无加速
- Linux: CUDA

#### `server/src/worker.rs` — 分析工作循环

这是应用的核心控制逻辑，采用**有限状态机**驱动：

```
┌─────────┐     首次识别      ┌──────────┐
│ Initial │───────────────────│ StartPos │
└────┬────┘                   └─────┬────┘
     │ 非初始局面                    │ 检测到走棋
     ▼                              ▼
┌─────────┐     轮换行棋      ┌───────────────┐
│ OurTurn │◄─────────────────►│ OpponentTurn  │
└─────────┘                   └───────────────┘
     ▲                              │
     │         无效变化>3次           │
     └──────── Invalid ◄────────────┘
```

状态说明：
- `Initial` — 启动后首次分析，确定阵营和初始局面
- `StartPos` — 识别到标准开局，等待首步落子
- `OurTurn` — 我方回合（已给出建议，等待对手走棋）
- `OpponentTurn` — 对方走棋后触发引擎分析
- `Invalid` — 识别异常，计数器超限后回退到 Initial

关键机制：
1. **预期棋盘对比**：如果当前棋盘 == 预期棋盘（即用户按建议走了），直接跳过重复分析
2. **二次确认**：检测到变化后 100ms 再截一次，确认局面稳定
3. **合法性校验**：验证棋子数量和位置是否符合象棋规则

#### `server/src/engine/mod.rs` — Pikafish 引擎

**版本：Pikafish 2025-01-10**

引用方式：**预编译二进制 + UCI 协议通信**（非源码集成）

各平台二进制：
| 文件 | 平台 | 架构 | 大小 |
|------|------|------|------|
| `pikafish-macos` | macOS | arm64 | 614KB |
| `pikafish-linux` | Linux | x86_64 | 708KB |
| `pikafish-windows.exe` | Windows | x86_64 | 1.4MB |
| `pikafish.nnue` | 通用 | - | 45MB |

职责：
- 启动 Pikafish 子进程，通过 stdin/stdout 通信 (UCI 协议)
- 提供 `search()` 方法执行局面分析
- 解析引擎输出（深度、分数、主变化线）

查询策略：
```
search(fen) {
    if 云库已启用 {
        result = 查询 chessdb.cn
        if result 成功 → 返回
        if result 无效局面 → 返回 None
    }
    // 云库失败或未启用，使用本地引擎
    position(fen)
    bestmove(depth, time) → 解析返回
}
```

UCI 命令流程：
```
setoption name EvalFile value pikafish.nnue
setoption name Threads value 4
setoption name Hash value 64
position fen <fen_string>
go depth 20 movetime 5000
→ 等待 "bestmove" 响应
```

#### `server/src/engine/chessdb.rs` — 云端开局库

职责：
- 调用 `chessdb.cn` API 查询已知局面的最佳走法
- 解析返回的 score/depth/pv 数据

接口格式：
```
GET http://www.chessdb.cn/chessdb.php?action=querypv&board=<FEN>
响应: score:123,depth:30,pv:h2e2|h9g7|i0h0|...
```

超时控制：默认 5 秒，可配置。

#### `server/src/chess.rs` — 棋局逻辑

职责：
- 棋盘数据结构：`[[char; 9]; 10]` 二维数组
- FEN 生成与解析
- 棋盘合法性校验（棋子数量、位置约束）
- 棋盘 diff（检测走棋）
- ICCS 坐标 → 中文走法转换
- 棋盘翻转（黑方视角）

中文走法转换规则：
- 红方用中文数字：一二三四五六七八九
- 黑方用阿拉伯数字：1 2 3 4 5 6 7 8 9
- 动作词：进/退/平
- 特殊标注：前/后（同列多子）

#### `server/src/config.rs` — 配置管理

职责：
- 从 `<config_dir>/xqlink/config.json` 加载/保存配置
- 提供 Tauri command 接口供前端修改引擎参数

配置项：
```json
{
    "loglevel": "INFO",
    "timer_interval": 100,
    "confirm_interval": 200,
    "engine": {
        "depth": 20,
        "time": 5000,
        "threads": 4,
        "hash": 64,
        "show_wdl": false,
        "chessdb_enabled": true,
        "chessdb_timeout": 5
    }
}
```

#### `server/src/common.rs` — 公共工具

职责：
- `detections_to_board()` — 将 YOLO 检测结果映射为 9×10 棋盘数组
- `detections_bound()` — 根据检测结果计算棋盘在图像中的边界矩形

#### `server/src/logger.rs` — 日志

使用 `tracing` + `tracing-subscriber` + `tracing-appender` 输出日志到文件。

---

## 数据流

### 启动流程

```
1. Tauri 启动 → 加载配置 → 创建 Pikafish 进程 → 初始化 ONNX Session
2. 前端加载 → 渲染初始棋盘 → 显示工具栏
```

### 监听流程

```
1. 用户点击"启动" → 前端调用 list_windows
2. 用户选择窗口 → 前端调用 start_listen(window)
3. 后端首次全图截屏 → YOLO 推理 → 计算棋盘边界 → 设置裁剪区域
4. 启动后台线程 → 进入 process_analysis_loop

循环:
    a. 按 timer_interval 间隔截屏
    b. YOLO 推理 → detections_to_board → board_fix
    c. 状态机判断:
        - 首次: 发送 position/mirror 事件 → 引擎分析 → 发送 analyse 事件
        - 无变化: 跳过
        - 对方走棋: 发送 move 事件 → 引擎分析 → 发送 analyse 事件
        - 我方走棋(符合预期): 发送 move 事件 → 轮换状态
        - 非预期变化: 确认 → 校验 → diff → 分析或忽略
```

### 事件通信

| 事件名 | 方向 | 数据 | 说明 |
|--------|------|------|------|
| `position` | 后端→前端 | `Position[]` | 更新整个棋盘 |
| `mirror` | 后端→前端 | `boolean` | 是否翻转棋盘 |
| `move` | 后端→前端 | `Changed` | 单步走棋动画 |
| `analyse` | 后端→前端 | `QueryResult` | 分析结果 |

---

## 核心算法

### YOLOv8 推理流程

```
输入图像 (任意尺寸)
    ↓ resize 到 640×640
    ↓ 归一化 [0,255] → [0,1]
    ↓ 转为 (1, 3, 640, 640) 张量
    ↓ ONNX Runtime 推理
    ↓ 输出张量 shape: (1, 20, N) → 转置为 (N, 20)
    ↓ 每行: [x, y, w, h, obj_conf, cls_0, cls_1, ..., cls_14]
    ↓ 过滤 conf < 0.7
    ↓ NMS (IOU < 0.5)
    ↓ 数量限制 (每类不超过规定上限)
    ↓
输出: Vec<Detection> { x0, y0, x1, y1, label, confidence }
```

### 棋盘构建

将检测到的棋子位置映射到 9×10 网格：
1. 根据所有检测框计算棋盘边界
2. 将每个检测框中心坐标映射到最近的网格位置
3. 通过 `board_fix` 确保红方在下、黑方在上（如果是黑方视角则翻转）

### 状态机转换

核心逻辑判断：
- 如果 `board == last_board` → 无变化，跳过
- 如果 `board == expect_board` → 用户按建议走了，轮换行棋方
- 否则 → diff 判断变化了几个格子：
  - 2格且有 from/to → 正常移动
  - 1格 → 可能是动画中间帧，计数容错
  - 其他 → 未知变化，重置状态

---

## 构建与运行

### 环境要求

- Node.js 18+
- pnpm
- Rust (nightly 或 stable，2024 edition)
- 平台对应的系统依赖

### 前端依赖安装

```bash
pnpm install
```

### 开发模式

```bash
pnpm tauri dev
```

### 生产构建

#### macOS

```bash
pnpm tauri build --config server/tauri.macos.conf.json
```

#### Windows (CPU)

```bash
pnpm run build:cpu
```

#### Windows (GPU，支持 CUDA/DirectML)

```bash
pnpm run build:gpu
```

#### Windows (CPU + 旋转模型)

```bash
pnpm run build:cpu:rotate
```

#### Windows (GPU + 旋转模型)

```bash
pnpm run build:gpu:rotate
```

### 构建产物

构建完成后，产物位于 `server/target/release/bundle/` 目录下。

### 所需外部资源

构建前需确保 `libs/` 目录下有以下文件：
- `large.onnx` 或 `rotate.onnx` — YOLOv8 模型权重
- `pikafish/pikafish` (或平台对应二进制) — 象棋引擎
- `pikafish/pikafish.nnue` — 引擎神经网络权重

---

## 配置说明

配置文件路径：`<系统配置目录>/xqlink/config.json`

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `loglevel` | string | "INFO" | 日志级别 |
| `timer_interval` | u64 | 100 | 截屏间隔 (ms) |
| `confirm_interval` | u64 | 200 | 二次确认等待 (ms) |
| `engine.depth` | usize | 20 | 搜索深度 |
| `engine.time` | usize | 5000 | 搜索限时 (ms) |
| `engine.threads` | usize | 4 | 引擎线程数 |
| `engine.hash` | usize | 64 | 哈希表大小 (MB) |
| `engine.show_wdl` | bool | false | 显示胜/和/负概率 |
| `engine.chessdb_enabled` | bool | true | 启用云库查询 |
| `engine.chessdb_timeout` | u64 | 5 | 云库超时 (秒) |

---

## 平台差异

| 特性 | macOS | Windows | Linux |
|------|-------|---------|-------|
| GPU 加速 | CoreML | CUDA + DirectML | CUDA |
| ONNX 加载 | 静态链接 | 动态加载 (load-dynamic) | 动态加载 |
| 引擎二进制 | pikafish-macos | pikafish-windows.exe | pikafish-linux |
| 窗口截屏 | xcap (Core Graphics) | xcap (Win32 API) | xcap (X11) |
| 构建配置 | tauri.macos.conf.json | tauri.windows.*.conf.json | tauri.linux.conf.json |
| 打包格式 | .dmg / .app | .msi (WiX) | .deb |

### 条件编译 Feature

| Feature | 作用 |
|---------|------|
| `gpu` | 启用 CUDA + DirectML 支持，复制动态库 |
| `rotate` | 使用旋转模型（适配翻转棋盘） |

### 打包策略

每个平台**只打包对应平台的二进制和运行时依赖**，不会跨平台混打。具体资源分配：

| 产物 | macOS 包 | Win CPU 包 | Win GPU 包 | Linux 包 |
|------|----------|------------|------------|----------|
| pikafish-macos | ✅ | - | - | - |
| pikafish-windows.exe | - | ✅ | ✅ | - |
| pikafish-linux | - | - | - | ✅ |
| pikafish.nnue (45MB) | ✅ | ✅ | ✅ | ✅ |
| windows-cpu/*.dll | - | ✅ | - | - |
| windows-gpu/*.dll | - | - | ✅ | - |
| libonnxruntime*.so | - | - | - | ✅ |
| ONNX 模型 | 编入二进制 | 编入二进制 | 编入二进制 | 编入二进制 |

通过 `--config` 参数指定对应平台的 Tauri 配置文件来控制打包内容。ONNX 模型则通过 `include_bytes!` 在编译期嵌入 Rust 二进制中，不作为外部资源打包。

---

## 扩展开发指南

### 添加新的 Tauri Command

1. 在 `server/src/` 对应模块中定义函数：
```rust
#[tauri::command]
pub async fn my_command(arg: String) -> Result<String, String> {
    // 实现
    Ok("result".to_string())
}
```

2. 在 `server/src/lib.rs` 的 `invoke_handler` 中注册：
```rust
.invoke_handler(tauri::generate_handler![
    // ...
    my_module::my_command,
])
```

3. 前端调用：
```typescript
import { invoke } from "@tauri-apps/api/core";
const result: string = await invoke("my_command", { arg: "hello" });
```

### 添加新的事件推送

后端发送：
```rust
app.emit("event_name", &data).unwrap();
```

前端监听：
```typescript
import { listen } from "@tauri-apps/api/event";
listen('event_name', (event) => {
    const data = event.payload;
});
```

### 替换/升级模型

1. 训练新的 YOLOv8 模型并导出为 ONNX 格式
2. 确保输出格式兼容（20维：4坐标 + 1置信度 + 15类别）
3. 替换 `libs/large.onnx` 或 `libs/rotate.onnx`
4. 如果类别有变化，修改 `yolo.rs` 中的 `LABELS` 和 `LIMIT`

### 添加新的引擎后端

实现以下接口即可替换引擎：
```rust
pub struct MyEngine {
    // ...
}

impl MyEngine {
    pub fn new(path: &Path) -> Self { /* ... */ }
    pub async fn search(&mut self, fen: &str, config: &EngineConfig) -> Option<QueryResult> { /* ... */ }
}
```

### 扩展为摄像头识别（参考思路）

如需支持实体棋盘识别：
1. 引入 `nokhwa` 或 `opencv` crate 获取摄像头帧
2. 增加透视变换预处理（矫正拍摄角度）
3. 复用现有 `yolo::predict()` 进行推理
4. 可能需要针对实体棋子重新训练模型

---

## 文件结构总览

```
chessboard/
├── docs/                      # 文档与截图
├── libs/                      # 外部资源
│   ├── config.json           # 构建配置
│   ├── large.onnx            # YOLOv8 模型
│   ├── rotate.onnx           # 旋转模型
│   └── pikafish/             # 引擎目录
│       ├── pikafish[-macos]  # 引擎二进制
│       └── pikafish.nnue     # NNUE 权重
├── public/                    # 静态资源
├── server/                    # Rust 后端 (Tauri)
│   ├── Cargo.toml            # Rust 依赖
│   ├── build.rs              # 构建脚本
│   ├── capabilities/         # Tauri 权限
│   ├── tauri.conf.json       # 基础 Tauri 配置
│   ├── tauri.macos.conf.json # macOS 配置
│   ├── tauri.linux.conf.json # Linux 配置
│   ├── tauri.windows.*.json  # Windows 配置
│   └── src/
│       ├── main.rs           # 入口
│       ├── lib.rs            # 应用初始化
│       ├── chess.rs          # 棋局逻辑
│       ├── common.rs         # 公共工具
│       ├── config.rs         # 配置管理
│       ├── listen.rs         # 窗口截屏
│       ├── logger.rs         # 日志
│       ├── worker.rs         # 分析工作循环
│       ├── yolo.rs           # YOLO 推理
│       └── engine/
│           ├── mod.rs        # 引擎封装
│           ├── command.rs    # 引擎进程管理
│           └── chessdb.rs    # 云库查询
├── src/                       # Vue 前端
│   ├── main.ts              # 前端入口
│   ├── App.vue              # 根组件
│   ├── assets/css/           # 棋盘样式
│   └── components/
│       ├── Toolbar.vue       # 工具栏
│       ├── Chessboard.vue    # 棋盘渲染
│       └── Analyse.vue       # 分析面板
├── index.html                 # HTML 入口
├── package.json               # 前端依赖
├── vite.config.ts             # Vite 配置
├── tsconfig.json              # TS 配置
└── README.md                  # 项目说明
```
