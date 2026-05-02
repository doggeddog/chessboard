# xiangqiai.com 功能分析 & 改进计划

## 背景

参考 [xiangqiai.com](https://xiangqiai.com/#/)，1:1 复刻其走棋和 Pikafish 分析功能，同时保留本项目特色的窗口自动识别棋盘能力。

---

## 当前已有 vs xiangqiai.com 功能对比

| 功能模块 | 当前状态 | xiangqiai.com | 差距 |
|---------|---------|---------------|------|
| **棋盘展示** | vschess 只读展示 | vschess 交互式 | 需要启用走棋交互 |
| **棋盘箭头** | 无 | 蓝色箭头标示建议走法 | 全新开发 |
| **走棋音效** | 无 | Howler.js 音效 | 全新开发 |
| **引擎分析** | 只显示最终 bestmove | 实时显示每个深度的分析过程 | 需要改造引擎通信 |
| **NPS/耗时** | 无 | 实时显示 NPS + 耗时 | 需要解析 info 行 |
| **WDL 概率** | 后端有 show_wdl 选项 | 显示胜/和/负概率 | 需要前端展示 |
| **云库面板** | 后端只用 `querypv` | 独立标签页,`queryall` 列出所有候选 | 后端+前端都要改 |
| **局势图表** | 无 | 折线图展示评分趋势 | 全新开发 |
| **走法历史** | 滚动文本日志 | 结构化列表(红黑交替) | 重写 |
| **棋谱导航** | 无 | `<<` `<` `>` `>>` 按钮 | 全新开发 |
| **翻转棋盘** | 有(mirror 事件) | 有 | 已有 |
| **复制 FEN** | 按钮存在但 disabled | 有 | 需实现 |
| **新建棋局** | 无 | 有 | 全新开发 |
| **编辑模式** | 无 | 有(自由摆子) | 全新开发 |
| **窗口识别** | 有(YOLO) | 无 | 保留(我们的特色) |

---

## 一、棋盘组件 — vschess 集成（已完成）

| 对比项 | 旧版 (Chessboard.vue) | 当前 (vschess) |
|--------|----------------------|----------------|
| 渲染方式 | DOM + CSS 类名切换 | DOM + CSS sprites |
| 棋子样式 | CSS 背景图 | 多套皮肤 (default/fc) |
| 棋盘背景 | CSS 格线 | 木纹纹理 |
| 走棋动画 | 无动画，瞬间切换 | 平滑移动动画 |
| 位置标记 | CSS 高亮 (`b-select`) | 小圆圈标记起止位置 |

### vschess 库信息

| 属性 | 信息 |
|------|------|
| 名称 | 微思象棋播放器 (vschess) |
| GitHub | https://github.com/FastLight126/vschess |
| 官网 | https://www.xiaxiangqi.com/vschess/ |
| 许可证 | LGPL-3.0 |
| 功能 | 打谱、对弈、拆解残局、棋谱解析、皮肤定制 |
| 渲染方式 | DOM + CSS sprites |

### vschess 资源依赖

vschess.js 不能单独使用，运行时会基于 `defaultPath`（自动检测为 vschess.js 所在目录）动态加载以下资源：

| 资源 | 路径 | 说明 |
|------|------|------|
| 全局样式 | `global.css` | 基础样式 |
| 棋子风格 | `style/{name}/style.css` + `*.png` | 棋子图片 + 棋盘背景 |
| 布局 | `layout/{name}/layout.css` + `*.png` | UI 布局资源 |
| 音效 | `sound/{name}/*.mp3` | 走棋/将军/胜负音效 |

当前 `public/vschess/` 包含完整仓库 clone（150+ 文件），其中 `develop/module/`（51 个源码文件）、多余 jQuery 版本、演示文件等可后续清理。

---

## 二、分析面板

### 当前 vs 目标

| 功能 | 当前 (Analyse.vue) | xiangqiai.com |
|------|-------------------|---------------|
| 最佳走法 | 显示中文走法 + 分数 + 深度 | 同上 |
| **多深度 PV 线** | 仅显示最终结果 | **实时显示每个深度的分析结果** |
| **NPS 显示** | 无 | **显示 NPS（每秒搜索节点）** |
| **耗时显示** | 无 | **显示搜索耗时** |
| **WDL 概率** | 后端有选项，前端未展示 | **显示胜/和/负概率** |
| **云库面板** | 后端 chessdb 只用 `querypv` | **独立标签页，`queryall` 列出所有候选走法及评分** |
| **局势图表** | 无 | **折线图展示评分变化趋势** |

### 关键技术差距：引擎通信

当前后端 `Engine::bestmove()` 方法的问题：
- 等待 `bestmove` 行才返回，中间的 `info depth ...` 行全部丢弃
- 无法实时推送分析过程到前端
- 只解析 `depth`、`score`、`pv`、`time`，缺少 `nodes`、`nps`、`wdl`

xiangqiai.com 的做法：
- 每收到一行 `info` 就实时更新 UI
- 显示当前搜索深度、节点数、NPS、耗时
- 搜索过程中 PV 线不断刷新

---

## 三、走法历史 & 导航

| 功能 | 当前 | xiangqiai.com |
|------|------|---------------|
| 走法记录 | 滚动日志 (NLog) | 结构化走法列表（红黑交替） |
| **棋谱导航** | 无 | **`<<` `<` `>` `>>` 按钮** |
| **变招标记** | 无 | **"变"字标示分支变化** |

---

## 四、工具栏功能

xiangqiai.com 顶部工具栏：
- 新建棋局
- 剪贴板操作
- 搜索开局
- 快速分析
- 翻转棋盘
- 局面分析图
- 复制 FEN
- 复制棋谱
- 外部链接
- 编辑模式
- 设置

---

## 实施计划 & 进度

### 第一阶段：替换棋盘为 vschess ✅ 已完成

- [x] 克隆 vschess 库到 `public/vschess/`
- [x] `index.html` 引入 jQuery + vschess.min.js
- [x] 重写 `Chessboard.vue`，用 vschess 替代旧 DOM/CSS 渲染
- [x] 实现 Position[] → FEN 转换，桥接后端 `position`/`move`/`mirror` 事件
- [x] CSS 覆盖隐藏 vschess 多余 UI（标签页、走法列表、控制栏等）
- [x] 窗口支持自由调整大小 (`resizable: true`)
- [x] 棋盘根据窗口高度自适应缩放（CSS transform + ResizeObserver）
- [x] 整体布局改为 Flexbox 响应式布局

#### 技术实现细节

**vschess 集成方式**：
- vschess 基于 jQuery，通过 `<script>` 标签全局引入（非 ES Module）
- 在 Vue 组件中通过 `declare const vschess: any` 访问全局变量
- 使用 `vschess.load("#selector", config)` 创建棋盘实例

**局面更新机制**：
- 前端维护 `board: string[10][9]` 内部状态
- `position` 事件 → 重建整个棋盘数组 → 转换为 FEN → 调用 `setNode` + `rebuildSituation` + `setBoardByStep(0)`
- `move` 事件 → 局部更新数组（清除起点、放置终点）→ 同上
- `mirror` 事件 → 调用 `chess.setTurn(isMirror ? 2 : 0)`

**FEN 转换**：
```
Position[] → board[10][9] → FEN 字符串
                              row9/row8/.../row0 w - - 0 1
```

**自适应缩放**：
- 棋盘原始尺寸：380×420px（9×40 + 20padding, 10×40 + 20padding）
- 缩放因子 `scale = Math.min(containerHeight / 420, 1.5)`
- 外层容器设为 `width: scaledW, height: scaledH`
- 内层用 `transform: scale(factor)` + `transform-origin: top left`
- ResizeObserver 监听容器变化，实时更新缩放

**vschess 配置**：
```javascript
{
    clickResponse: 0,   // 禁用交互（仅展示）
    sound: false,       // 关闭音效
    moveTips: false,    // 关闭走子提示
    saveTips: false,    // 关闭保存提示
}
```

### 第二阶段：引擎分析增强 🔄 进行中

核心目标：从"只显示最终结果"改为"实时推送分析过程"。

- [ ] **后端：实时推送 info 行**
  - [ ] 改造 `Engine::bestmove()` → `Engine::go()`，每收到 `info` 行就 `app.emit("analyse", ...)`
  - [ ] 解析更多字段：`nodes`、`nps`、`wdl`（胜/和/负概率）
  - [ ] 支持 `go infinite` + `stop` 模式（持续分析直到手动停止）
- [ ] **前端：分析面板重写**
  - [ ] 实时显示当前深度、分数、NPS、耗时
  - [ ] 显示完整 PV 线（中文走法）
  - [ ] 显示 WDL 概率条
  - [ ] 分析数据结构从 `Analyse` 接口扩展（增加 nodes、nps、wdl 字段）

#### 涉及文件

| 文件 | 变更说明 |
|------|---------|
| `server/src/engine/mod.rs` | 改造 search/bestmove 方法，逐行推送 info |
| `server/src/worker.rs` | 适配新的引擎通信方式 |
| `src/components/Analyse.vue` | 重写分析面板 UI |

### 第三阶段：棋盘交互 + 箭头指示

- [ ] 在"独立分析"模式下启用 vschess 的 `clickResponse`（可走棋）
- [ ] 走棋后自动触发引擎分析
- [ ] 棋盘上绘制箭头标示最佳走法（利用 vschess `guessArrow` 或 Canvas 叠加）
- [ ] 走棋音效（利用 vschess 内置 `sound/default/*.mp3`）

### 第四阶段：云库 + 局势图表

- [ ] **云库面板**
  - [ ] 后端新增 `queryall` API 调用
  - [ ] 前端新增"云库"标签页，展示所有候选走法及评分
  - [ ] 按评分排序，标示行棋方
- [ ] **局势评分图表**
  - [ ] 折线图组件（Chart.js 或 ECharts）
  - [ ] X 轴=步数，Y 轴=评分，实时更新

### 第五阶段：走法历史 + 棋谱导航

- [ ] 走法记录改为结构化列表（红黑交替）
- [ ] 棋谱导航按钮：`<<` `<` `>` `>>`
- [ ] 支持点击某步回看局面
- [ ] 支持变招分支

### 第六阶段：工具栏扩展

- [ ] 复制 FEN — 当前局面 FEN 复制到剪贴板
- [ ] 新建棋局 — 重置棋盘为初始局面
- [ ] 编辑模式 — 自由摆放棋子，设置局面后分析
- [ ] 搜索开局 — 开局库检索

### 不需要复刻的部分

- **Pikafish Wasm 编译** — 我们用原生进程调用，性能更好
- **PWA/ServiceWorker** — Tauri 桌面应用不需要
- **多线程 SharedArrayBuffer** — 原生引擎已有多线程支持

---

## 变更文件清单（第一阶段 — 已完成）

| 文件 | 变更说明 |
|------|---------|
| `public/vschess/` | 新增：克隆自 GitHub，包含 jQuery、棋盘样式、音效等 |
| `index.html` | 修改：添加 jQuery + vschess.min.js 脚本引用 |
| `src/App.vue` | 重写：absolute 定位改为 Flexbox 响应式布局 |
| `src/components/Chessboard.vue` | 重写：DOM/CSS 棋盘改为 vschess 实例 + 自适应缩放 |
| `src/components/Analyse.vue` | 修改：移除 b-select DOM 操作、适配 flex 布局 |
| `server/tauri.conf.json` | 修改：窗口可缩放、可最大化、设置最小尺寸 |
| `docs/xiangqiai-feature-analysis.md` | 新增：本文档 |
