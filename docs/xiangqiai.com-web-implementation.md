# 浏览器中运行 Pikafish：xiangqiai.com 技术分析

## 概述

[xiangqiai.com](https://xiangqiai.com/) 是一个在线象棋 AI 分析网站，它在**浏览器中直接运行 Pikafish 引擎**，无需服务器端算力支持。这是通过将 C++ 引擎编译为 **WebAssembly (Wasm)** 实现的。

## 整体架构

```
用户的浏览器
┌─────────────────────────────────────────────────────────┐
│                                                         │
│  主线程 (Main Thread)                                    │
│  ┌───────────────────────────────────────────────────┐  │
│  │  前端 SPA 应用                                     │  │
│  │  ┌─────────────┐  ┌──────────────┐  ┌──────────┐ │  │
│  │  │  棋盘 UI    │  │  分析面板    │  │  走法列表 │ │  │
│  │  │  (Canvas/   │  │  (评估分数   │  │  (PV显示) │ │  │
│  │  │   SVG/DOM)  │  │   深度/NPS)  │  │          │ │  │
│  │  └─────────────┘  └──────────────┘  └──────────┘ │  │
│  │                                                   │  │
│  │  ┌─────────────────────────────────────────────┐  │  │
│  │  │  UCI 控制器                                  │  │  │
│  │  │  - 组装 UCI 命令字符串                       │  │  │
│  │  │  - 解析引擎输出 (info/bestmove)             │  │  │
│  │  │  - 更新 UI 状态                              │  │  │
│  │  └──────────────────┬──────────────────────────┘  │  │
│  └─────────────────────┼─────────────────────────────┘  │
│                        │ postMessage()                   │
│                        ▼                                 │
│  Web Worker 线程                                         │
│  ┌───────────────────────────────────────────────────┐  │
│  │                                                   │  │
│  │  Emscripten JS 胶水代码                           │  │
│  │  ┌─────────────────────────────────────────────┐  │  │
│  │  │  Module 对象                                │  │  │
│  │  │  - stdin: ← 接收 postMessage 的 UCI 命令    │  │  │
│  │  │  - print: → 通过 postMessage 发回输出       │  │  │
│  │  │  - onRuntimeInitialized: 引擎初始化完成回调 │  │  │
│  │  └──────────────────┬──────────────────────────┘  │  │
│  │                     │                             │  │
│  │  ┌──────────────────▼──────────────────────────┐  │  │
│  │  │  pikafish.wasm                              │  │  │
│  │  │  ┌────────────────────────────────────────┐ │  │  │
│  │  │  │  Pikafish C++ 引擎 (编译为 Wasm)       │ │  │  │
│  │  │  │  - 搜索算法 (Alpha-Beta)               │ │  │  │
│  │  │  │  - NNUE 评估                            │ │  │  │
│  │  │  │  - 走法生成                             │ │  │  │
│  │  │  │  - UCI 循环 (getline → 处理 → cout)     │ │  │  │
│  │  │  └────────────────────────────────────────┘ │  │  │
│  │  │                                             │  │  │
│  │  │  pikafish.nnue (NNUE 神经网络权重文件)       │  │  │
│  │  └─────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────┘  │
│                                                         │
└─────────────────────────────────────────────────────────┘
                        ↕ 无需服务器参与
```

## 关键技术详解

### 1. WebAssembly 编译

Pikafish 本身是 C++ 项目，通过 **Emscripten** 编译工具链转换为可在浏览器运行的格式。

#### 编译流程

```
Pikafish C++ 源码 (src/)
        │
        │  Emscripten (emcc/em++)
        │  替代 gcc/g++ 作为编译器
        ▼
┌───────────────────┐   ┌───────────────────┐
│  pikafish.wasm    │   │  pikafish.js      │
│  (WebAssembly     │   │  (JavaScript      │
│   二进制模块)      │   │   胶水代码)       │
└───────────────────┘   └───────────────────┘
```

编译过程大致等价于：

```bash
# 普通编译（生成本地可执行文件）
g++ -O3 -o pikafish src/*.cpp

# Wasm 编译（生成浏览器可运行的模块）
em++ -O3 -o pikafish.js src/*.cpp \
  -s WASM=1 \
  -s ALLOW_MEMORY_GROWTH=1 \
  -s MODULARIZE=1 \
  --embed-file pikafish.nnue
```

#### 产出文件

| 文件 | 大小 (估计) | 说明 |
|------|------------|------|
| `pikafish.wasm` | ~2-5 MB | 引擎逻辑的 WebAssembly 二进制 |
| `pikafish.js` | ~100-200 KB | Emscripten 生成的胶水代码，负责加载 wasm 和桥接 |
| `pikafish.nnue` | ~30-60 MB | NNUE 神经网络权重文件（可能被嵌入或单独加载） |

### 2. Web Worker 隔离

**为什么需要 Web Worker？**

Pikafish 的搜索是 CPU 密集型的，如果在主线程运行会阻塞 UI 渲染，导致页面完全卡死。Web Worker 提供了独立的线程。

```javascript
// 主线程：创建 Worker
const worker = new Worker('pikafish-worker.js');

// 主线程：发送 UCI 命令
worker.postMessage('position startpos');
worker.postMessage('go depth 20');

// 主线程：接收引擎输出
worker.onmessage = function(event) {
    const line = event.data;
    // line 就是引擎的 stdout 输出
    // 例如: "info depth 10 score cp 22 nodes 12888 ..."
    // 例如: "bestmove h2e2 ponder h9g7"
    parseEngineOutput(line);
};
```

### 3. stdin/stdout 重定向

这是最关键的技术点：如何让 C++ 的 `std::cin` / `std::cout` 在浏览器中工作。

#### 原生 Pikafish 的通信方式

```
终端 stdin ──→ std::getline(std::cin, cmd) ──→ 引擎处理
引擎处理   ──→ std::cout << "bestmove ..." ──→ 终端 stdout
```

#### Wasm 版的通信方式

Emscripten 提供了 `Module` 对象来重定向 I/O：

```javascript
// Worker 内部的代码 (pikafish-worker.js)

var Module = {
    // 重定向 stdout：引擎的 cout 输出会调用这个函数
    print: function(text) {
        // 将引擎输出通过 postMessage 发回主线程
        postMessage(text);
    },

    // 重定向 stderr
    printErr: function(text) {
        console.error('Engine:', text);
    },

    // Wasm 模块初始化完成后的回调
    onRuntimeInitialized: function() {
        console.log('Pikafish engine ready');
    }
};

// 接收主线程发来的 UCI 命令
var inputQueue = [];

onmessage = function(event) {
    // 将命令放入队列
    inputQueue.push(event.data + '\n');  // 注意：需要加换行符！
};

// Emscripten 的 stdin 重定向
Module.stdin = function() {
    // 当引擎调用 getline(cin, cmd) 时
    // Emscripten 会不断调用这个函数获取字符
    if (inputQueue.length > 0) {
        var char = inputQueue[0].charCodeAt(0);
        inputQueue[0] = inputQueue[0].substring(1);
        if (inputQueue[0].length === 0) {
            inputQueue.shift();
        }
        return char;
    }
    return null;  // 没有更多输入，等待
};

// 加载 Wasm 模块
importScripts('pikafish.js');
```

#### 数据流全程追踪

```
用户点击棋盘走子
    │
    │  前端 JS 构造 UCI 命令
    ▼
"position startpos moves h2e2 h9g7"
    │
    │  worker.postMessage(command)
    ▼
Worker onmessage 接收
    │
    │  放入 inputQueue
    ▼
Module.stdin() 被 Emscripten 调用
    │
    │  逐字符返回命令字符串
    ▼
C++ getline(std::cin, cmd) 获得完整命令
    │
    │  引擎处理（搜索/评估）
    ▼
C++ std::cout << "info depth 10 ..."
    │
    │  Emscripten 调用 Module.print()
    ▼
Worker postMessage(text) 发回主线程
    │
    │  worker.onmessage 接收
    ▼
前端解析输出，更新 UI
    │
    ▼
用户看到：深度 10，评估分 +22，最佳走法 h2e2
```

### 4. NNUE 网络文件加载

NNUE 网络文件 (`pikafish.nnue`) 通常有以下加载方式：

| 方式 | 说明 | 优缺点 |
|------|------|--------|
| **嵌入 Wasm** | 编译时用 `--embed-file` 嵌入 | 加载简单，但 Wasm 文件很大 |
| **预加载** | 用 `--preload-file` 生成 `.data` 文件 | 分离文件，可缓存 |
| **运行时加载** | Fetch API 下载后传给引擎 | 最灵活，可更换网络 |

### 5. 多线程支持：Wasm 天然支持多线程吗？

**Wasm 本身不直接提供多线程原语，但可以通过以下机制实现多线程。**

#### 多线程的底层原理

```
原生 Pikafish 多线程 (C++ pthreads)
┌──────────────────────────────────────────┐
│  进程                                     │
│  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐    │
│  │Thread│ │Thread│ │Thread│ │Thread│    │
│  │  0   │ │  1   │ │  2   │ │  3   │    │
│  └──┬───┘ └──┬───┘ └──┬───┘ └──┬───┘    │
│     └────────┴────────┴────────┘         │
│              共享内存 (堆)                 │
└──────────────────────────────────────────┘

Wasm 多线程 (Emscripten pthreads → Web Workers)
┌──────────────────────────────────────────┐
│  浏览器标签页                              │
│  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐    │
│  │Worker│ │Worker│ │Worker│ │Worker│    │
│  │  0   │ │  1   │ │  2   │ │  3   │    │
│  │(主)  │ │      │ │      │ │      │    │
│  └──┬───┘ └──┬───┘ └──┬───┘ └──┬───┘    │
│     └────────┴────────┴────────┘         │
│        SharedArrayBuffer (共享内存)        │
└──────────────────────────────────────────┘
```

关键技术组件：

| 组件 | 作用 | 说明 |
|------|------|------|
| **Web Workers** | 提供独立线程 | 浏览器的线程 API，每个 Worker 有独立的执行上下文 |
| **SharedArrayBuffer** | 提供共享内存 | 允许多个 Worker 访问同一块内存，对标 C++ 的共享堆内存 |
| **Atomics** | 提供原子操作 | 对标 C++ 的 `std::atomic`、mutex、条件变量 |
| **Emscripten** | 粘合层 | 将 C++ 的 `pthread_create()` 翻译为创建 Web Worker |

#### 具体工作流程

1. **编译阶段**：Emscripten 将 C++ 的 `pthread` 调用转换为 Web Worker 操作

```
C++ 代码                          Emscripten 翻译
───────────────                   ──────────────────────────
pthread_create()          →       new Worker('pikafish.js')
pthread_mutex_lock()      →       Atomics.wait()
pthread_mutex_unlock()    →       Atomics.notify()
malloc() (共享堆)         →       SharedArrayBuffer 上分配
```

2. **运行阶段**：引擎调用 `Threads value 8` 时

```
Pikafish C++ 代码:
    ThreadPool::set(8)
        → pthread_create() × 7 (主线程 + 7 工作线程)

Emscripten 翻译后:
    创建 7 个 Web Worker
    每个 Worker 加载 pikafish.js
    所有 Worker 共享同一个 SharedArrayBuffer
```

3. **内存模型**：

```
SharedArrayBuffer (Wasm 线性内存)
┌────────────────────────────────────────────┐
│  [0 - 16MB]  引擎代码和栈                    │
│  [16MB - ...]  堆内存                        │
│    ├─ 置换表 (384MB Hash)                    │
│    ├─ NNUE 网络权重                          │
│    ├─ 搜索状态 (各线程的 Worker 数据)         │
│    └─ 其他动态分配                           │
└────────────────────────────────────────────┘
     ↑           ↑           ↑           ↑
   Worker 0   Worker 1   Worker 2   Worker 3
   (所有 Worker 看到相同的内存内容)
```

#### 为什么 pikafish.js 要加载 8 次？

因为 **Web Workers 没有代码共享机制**——每个 Worker 是一个独立的 JavaScript 执行环境。

```
主线程:
    new Worker('pikafish.js')  → Worker 0 (主搜索线程)
    new Worker('pikafish.js')  → Worker 1
    new Worker('pikafish.js')  → Worker 2
    ...
    new Worker('pikafish.js')  → Worker 7
```

每个 Worker 创建时：
1. **加载 `pikafish.js`**：这是 Emscripten 生成的胶水代码，包含 Wasm 模块加载逻辑
2. **实例化 Wasm 模块**：每个 Worker 创建自己的 Wasm 实例
3. **附加到共享内存**：所有 Worker 的 Wasm 实例使用同一个 SharedArrayBuffer

```javascript
// Emscripten 生成的 Worker 入口 (简化版)
// 这就是 pikafish.js 中的关键逻辑

self.onmessage = function(e) {
    if (e.data.cmd === 'run') {
        // 从主线程接收共享内存
        var sharedBuffer = e.data.buffer;  // SharedArrayBuffer

        // 用共享内存初始化 Wasm 模块
        Module.wasmMemory = new WebAssembly.Memory({
            initial: ...,
            maximum: ...,
            shared: true  // 关键：标记为共享内存
        });

        // 实例化 Wasm（代码相同，内存共享）
        WebAssembly.instantiate(wasmBinary, {
            env: { memory: Module.wasmMemory }
        });

        // 执行 pthread 入口函数
        Module._emscripten_thread_init(e.data.threadId);
    }
};
```

所以虽然 `pikafish.js` 的代码被加载了 8 次，但：
- **代码是相同的**（浏览器会从缓存读取，第 2-8 次大小为 0）
- **内存是共享的**（通过 SharedArrayBuffer）
- **每个 Worker 运行独立的搜索线程**（对应 C++ 的 ThreadPool 中的 Worker）

#### 安全限制

SharedArrayBuffer 因为 Spectre 漏洞，需要跨域隔离：

```
服务器必须返回以下 HTTP 头:
Cross-Origin-Embedder-Policy: require-corp     ← 阻止非同源资源
Cross-Origin-Opener-Policy: same-origin        ← 隔离浏览器上下文

没有这些头 → SharedArrayBuffer 不可用 → 只能单线程
```

xiangqiai.com 正确设置了这些头部（实测确认），所以能使用 8 线程。

#### 如果不支持多线程？

引擎会回退到单线程模式：
- `Threads` 选项锁定为 1
- 不创建额外 Worker
- 性能显著降低（约为多线程版的 1/N）
- 旧版浏览器或未设置 COOP/COEP 的网站会出现这种情况

### 6. 性能对比

| 平台 | 预估性能 | 说明 |
|------|---------|------|
| **原生 (Apple M2)** | ~100% | 直接编译的二进制 |
| **原生 (x86 AVX2)** | ~100% | 利用 SIMD 指令集 |
| **Wasm (单线程)** | ~30-50% | 无 SIMD，单线程 |
| **Wasm + SIMD** | ~50-70% | 浏览器支持 Wasm SIMD |
| **Wasm + SIMD + 多线程** | ~60-80% | 最佳浏览器配置 |

浏览器版本的性能损失主要来自：
- Wasm 的间接层开销
- SIMD 指令不如原生 AVX2/NEON 丰富
- 浏览器安全策略限制（如 SharedArrayBuffer 需要特殊头部）
- 内存分配开销

## 前端实现要点

### UCI 命令构造

```javascript
class UCIController {
    constructor(worker) {
        this.worker = worker;
        this.worker.onmessage = (e) => this.handleOutput(e.data);
    }

    // 初始化引擎
    init() {
        this.send('uci');
        // 等待 uciok
    }

    // 设置局面
    setPosition(fen, moves = []) {
        if (fen === 'startpos') {
            let cmd = 'position startpos';
            if (moves.length > 0) cmd += ' moves ' + moves.join(' ');
            this.send(cmd);
        } else {
            let cmd = `position fen ${fen}`;
            if (moves.length > 0) cmd += ' moves ' + moves.join(' ');
            this.send(cmd);
        }
    }

    // 开始分析
    analyze(depth = 20) {
        this.send(`go depth ${depth}`);
    }

    // 停止分析
    stop() {
        this.send('stop');
    }

    // 发送命令
    send(cmd) {
        this.worker.postMessage(cmd);
    }

    // 处理引擎输出
    handleOutput(line) {
        if (line.startsWith('info') && line.includes('depth')) {
            this.parseInfo(line);
        } else if (line.startsWith('bestmove')) {
            this.parseBestMove(line);
        }
    }

    parseInfo(line) {
        const tokens = line.split(' ');
        const info = {};
        for (let i = 0; i < tokens.length; i++) {
            switch (tokens[i]) {
                case 'depth': info.depth = parseInt(tokens[++i]); break;
                case 'score':
                    info.scoreType = tokens[++i]; // 'cp' or 'mate'
                    info.scoreValue = parseInt(tokens[++i]);
                    break;
                case 'nodes': info.nodes = parseInt(tokens[++i]); break;
                case 'nps': info.nps = parseInt(tokens[++i]); break;
                case 'pv': info.pv = tokens.slice(i + 1); i = tokens.length; break;
            }
        }
        // 更新 UI
        this.onInfo?.(info);
    }

    parseBestMove(line) {
        const parts = line.split(' ');
        const bestmove = parts[1];
        const ponder = parts[3] || null;
        this.onBestMove?.(bestmove, ponder);
    }
}
```

### 走法坐标转换

UCI 走法格式（如 `h2e2`）需要转换为棋盘 UI 坐标：

```javascript
function uciToBoard(uciMove) {
    // UCI: h2e2 → 从 (7,2) 到 (4,2)
    const fromCol = uciMove.charCodeAt(0) - 'a'.charCodeAt(0);  // h → 7
    const fromRow = parseInt(uciMove[1]);                        // 2
    const toCol   = uciMove.charCodeAt(2) - 'a'.charCodeAt(0);  // e → 4
    const toRow   = parseInt(uciMove[3]);                        // 2
    return { from: [fromCol, fromRow], to: [toCol, toRow] };
}

function boardToUci(fromCol, fromRow, toCol, toRow) {
    return String.fromCharCode('a'.charCodeAt(0) + fromCol) + fromRow
         + String.fromCharCode('a'.charCodeAt(0) + toCol) + toRow;
}
```

## 实际观测数据（Chrome DevTools 分析）

以下数据通过 Chrome DevTools 对 xiangqiai.com 的实际运行抓取获得。

### 加载的引擎文件

| 文件 | 路径 | 大小 |
|------|------|------|
| `pikafish.js` | `/engine/main_20240816v7/multi_simd_relaxed/pikafish.js` | ~61 KB |
| `pikafish.wasm` | `/engine/main_20240816v7/multi_simd_relaxed/pikafish.wasm` | ~510 KB |
| `pikafish.data` | `/engine/main_20240816v7/data/pikafish.data` | ~3.9 MB |

路径 `multi_simd_relaxed` 表明使用了 **Wasm SIMD + Relaxed SIMD** 指令集。`pikafish.data` 是 Emscripten 预加载的文件系统数据，包含 NNUE 网络文件。

### HTTP 响应头

服务器确实设置了多线程所需的跨域头部：

```
Cross-Origin-Embedder-Policy: require-corp
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Resource-Policy: cross-origin
```

这使得 `SharedArrayBuffer` 可用，允许 Wasm 使用多线程。

### 浏览器环境

```
SharedArrayBuffer: 可用 ✓
WebAssembly: 可用 ✓
ServiceWorker: 可用 ✓
硬件并发数: 8 核
```

### 引擎版本与 UCI 初始化

控制台日志记录了引擎的启动过程（`>` 表示引擎输出，`<` 表示发送给引擎的命令）：

```
> Pikafish dev-20240816-b41514ac by the Pikafish developers (see AUTHORS file)
< uci
> id name Pikafish dev-20240816-b41514ac
> id author the Pikafish developers (see AUTHORS file)
> option name Threads type spin default 1 min 1 max 1024
> option name Hash type spin default 16 min 1 max 33554432
> option name MultiPV type spin default 1 min 1 max 128
> option name Skill Level type spin default 20 min 0 max 20
> option name Mate Threat Depth type spin default 1 min 0 max 10
> option name Repetition Rule type combo default AsianRule ...
> option name EvalFile type string default pikafish.nnue
> uciok
```

### 网站自动配置的选项

```
< setoption name Threads value 8           ← 使用所有 8 核
< setoption name Hash value 384            ← 384 MB 哈希表
< setoption name MultiPV value 1           ← 单条最佳变例
< setoption name Skill Level value 20      ← 最高棋力
< setoption name Repetition Rule value AsianRule
< setoption name Sixty Move Rule value false
< setoption name UCI_ShowWDL value true     ← 显示胜/和/负概率
< setoption name Mate Threat Depth value 6 ← 杀棋威胁检测深度
```

可以看到网站自动将线程数设为 8（等于 `navigator.hardwareConcurrency`），充分利用了多核。

### 前端技术栈

- **框架**: Vue.js
- **棋盘**: `vschess` 库 + Canvas 渲染
- **音效**: Howler.js
- **棋谱格式**: 支持 XQF 格式
- **云库**: 调用 `chessdb.cn` API 查询开局库
- **服务器**: Nginx + CDN 缓存
- **PWA**: 使用 Workbox 支持离线访问

### `pikafish.js` 被加载多次的原因

网络请求中 `pikafish.js` 被加载了 8+ 次，这是因为 Emscripten 多线程模式下，每个 Worker 线程（`pthread`）都需要加载一份胶水代码。8 核 = 1 主线程 + 7 个 Worker 线程，符合 `Threads value 8` 的设置。

### 棋盘 UI：vschess

xiangqiai.com 的棋盘界面基于 **[vschess](https://github.com/FastLight126/vschess)** 项目。

| 属性 | 信息 |
|------|------|
| **名称** | vschess — 中国象棋 Chinese Chess Web UI |
| **GitHub** | <https://github.com/FastLight126/vschess> |
| **许可证** | LGPL-3.0 |
| **功能** | 打谱、对弈、拆解残局、棋谱解析、皮肤定制 |
| **渲染方式** | Canvas |

### 前端完整技术栈

| 模块 | 技术 | 加载的文件 |
|------|------|-----------|
| **应用框架** | Vue.js (SPA) | `index.js` |
| **主视图** | 自定义组件 | `MainView.js` |
| **棋盘 UI** | vschess + 自定义封装 | `ChessBoard.js`, `vschess.function.js` |
| **棋谱格式** | XQF 解析器 | `xqf.js` |
| **音效** | Howler.js | `howler.js` |
| **引擎** | Pikafish Wasm | `pikafish.js` (×8), `pikafish.wasm`, `pikafish.data` |
| **PWA** | Workbox | `workbox-window.prod.es5.js` |
| **统计** | Google Analytics | `gtag/js` |
| **云库** | chessdb.cn API | HTTP 请求 |

### 云库 API (chessdb.cn)

xiangqiai.com 使用 [chessdb.cn](https://www.chessdb.cn/) 的云库 API 提供开局库查询功能。

#### 什么是云库？

云库是一个大型象棋开局数据库，包含数亿个分析过的局面，可以快速返回已知局面的最佳走法和评估分数，无需引擎实时计算。

#### API 信息

| 属性 | 信息 |
|------|------|
| **接口地址** | `http://www.chessdb.cn/chessdb.php` |
| **协议** | HTTP RESTful API |
| **费用** | **免费** |
| **限制** | 每 IP 每 24 小时最多 10 万次查询 |
| **文档** | [中文](https://www.chessdb.cn/cloudbook_api.html) / [英文](https://www.chessdb.cn/cloudbook_api_en.html) |

#### API 操作列表

| 操作 (action) | 说明 |
|---------------|------|
| `queryall` | 查询所有已知着法及评分 |
| `querybest` | 查询最佳着法 |
| `query` | 查询随机候选着法 |
| `queryscore` | 查询局面评估分值 |
| `querypv` | 查询思考细节（深度、PV 线） |
| `queryrule` | 查询棋规裁定（是否犯规） |
| `queue` | 提交局面到后台计算 |
| `store` | 提交学习局面着法 |

#### xiangqiai.com 的实际调用示例

从网络请求中观测到的真实 API 调用：

```
GET https://www.chessdb.cn/chessdb.php
    ?action=queryall
    &learn=1
    &showall=0
    &board=2baka1r1/4n4/4b1nc1/p1P1N1C1p/3r4c/7R1/P3P4/4C4/9/1RBAKAB2%20w
```

参数说明：
- `action=queryall` — 查询该局面所有已知走法
- `learn=1` — 自动学习（贡献数据）
- `showall=0` — 只返回已知走法
- `board=...` — FEN 格式的局面

返回结果包含走法列表和评分（如页面上显示的"车八进一: 6", "车八进四: 0" 等）。

#### 默认的数据贡献机制

`learn=1` 参数意味着每次查询都会触发云库的自动学习——云库会分析并存储该局面相关的走法。如果设为 `learn=0`，则仅返回单个最佳走法。这是一个**众包模式**：使用者查询的同时也在贡献数据，使云库不断扩大。

## 与原生版本的通信方式对比

| 方面 | 原生 (命令行/GUI) | 浏览器 (Wasm) |
|------|-------------------|--------------|
| **进程模型** | 独立进程 | Web Worker |
| **stdin** | 操作系统管道 | Module.stdin 回调 |
| **stdout** | 操作系统管道 | Module.print 回调 |
| **通信方式** | pipe / 文件描述符 | postMessage |
| **UCI 命令** | 相同 | 相同 |
| **输出格式** | 相同 | 相同 |
| **引擎代码** | 相同 (C++) | 相同 (C++ 编译为 Wasm) |
| **线程** | OS 原生线程 | SharedArrayBuffer + Worker |

核心优势：**UCI 协议层完全不变**。引擎内部的 `getline(std::cin)` 和 `std::cout` 被 Emscripten 透明地重定向，引擎代码本身不需要任何修改就能在浏览器中运行。
