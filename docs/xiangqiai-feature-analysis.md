# xiangqiai.com 功能分析 & 改进计划

## 背景

参考 [xiangqiai.com](https://xiangqiai.com/#/)，计划将当前象棋棋盘替换为 [vschess](https://github.com/FastLight126/vschess) 库，并添加一系列新功能。

---

## 一、棋盘组件 — 替换为 vschess

| 对比项 | 旧版 (Chessboard.vue) | xiangqiai.com (vschess) |
|--------|----------------------|------------------------|
| 渲染方式 | DOM + CSS 类名切换 | Canvas 渲染 |
| 棋子样式 | CSS 背景图 | Canvas 绘制，多套皮肤 |
| 棋盘背景 | CSS 格线 | 木纹纹理 |
| 走棋动画 | 无动画，瞬间切换 | 平滑移动动画 |
| **指示箭头** | 无 | **蓝色箭头标示建议走法** |
| 位置标记 | CSS 高亮 (`b-select`) | 小圆圈标记起止位置 |
| 走棋音效 | 无 | 有音效 (Howler.js) |
| 棋谱格式 | 无 | 支持 PGN/XQF 等多种格式 |
| 打谱/回放 | 无 | 支持 (`<<` `<` `>` `>>` 导航) |

### vschess 库信息

| 属性 | 信息 |
|------|------|
| 名称 | 微思象棋播放器 (vschess) |
| GitHub | https://github.com/FastLight126/vschess |
| 官网 | https://www.xiaxiangqi.com/vschess/ |
| 许可证 | LGPL-3.0 |
| 功能 | 打谱、对弈、拆解残局、棋谱解析、皮肤定制 |
| 渲染方式 | DOM + CSS sprites (非 Canvas) |

---

## 二、分析面板增强

| 功能 | 当前 (Analyse.vue) | xiangqiai.com |
|------|-------------------|---------------|
| 最佳走法 | 显示中文走法 + 分数 + 深度 | 同上 |
| **多深度 PV 线** | 仅显示最终结果 | **实时显示每个深度的分析结果** |
| **NPS 显示** | 无 | **显示 NPS（每秒搜索节点）** |
| **耗时显示** | 无 | **显示搜索耗时** |
| **云库面板** | 后端已有 chessdb 集成，前端无独立展示 | **独立标签页，列出所有候选走法及评分** |
| **局势图表** | 无 | **折线图展示评分变化趋势** |
| **走法注释** | 无 | **可给走法添加注释** |

### 云库面板详情

xiangqiai.com 的云库面板：
- 调用 `chessdb.cn` API (`action=queryall`)
- 显示当前局面所有已知走法及评分
- 按评分排序
- 标示当前行棋方（红方/黑方）

我们的后端已有 chessdb 集成（`server/src/engine/chessdb.rs`），但目前只用于查询最佳走法（`querypv`），需要扩展为 `queryall` 并在前端展示。

### 局势图表详情

- 折线图，X 轴为步数，Y 轴为评分
- 实时更新
- 显示当前评分数值

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

### 第二阶段：添加指示箭头

- [ ] 利用 vschess 的绘图能力或 Canvas 叠加层，在棋盘上绘制箭头
- [ ] 从后端 `analyse` 事件读取 PV 线（ICCS 格式如 `h2e2`），转换为棋盘坐标
- [ ] 支持显示/隐藏箭头

### 第三阶段：增强分析面板

- [ ] 添加"云库"标签页
  - [ ] 后端扩展 chessdb API 为 `queryall`
  - [ ] 前端展示所有候选走法及评分
- [ ] 添加"局势"标签页
  - [ ] 折线图展示评分变化
- [ ] 增强引擎分析输出
  - [ ] 多深度 PV 线
  - [ ] NPS、耗时显示

### 第四阶段：走法历史与导航

- [ ] 走法记录改为结构化列表
- [ ] 添加棋谱前后翻阅功能
- [ ] 支持变招分支

### 第五阶段：工具栏扩展

- [ ] 复制 FEN / 棋谱
- [ ] 搜索开局
- [ ] 编辑模式
- [ ] 走棋音效

---

## 变更文件清单（第一阶段）

| 文件 | 变更说明 |
|------|---------|
| `public/vschess/` | 新增：克隆自 GitHub，包含 jQuery、棋盘样式、音效等 |
| `index.html` | 修改：添加 jQuery + vschess.min.js 脚本引用 |
| `src/App.vue` | 重写：absolute 定位改为 Flexbox 响应式布局 |
| `src/components/Chessboard.vue` | 重写：DOM/CSS 棋盘改为 vschess 实例 + 自适应缩放 |
| `src/components/Analyse.vue` | 修改：移除 b-select DOM 操作、适配 flex 布局 |
| `server/tauri.conf.json` | 修改：窗口可缩放、可最大化、设置最小尺寸 |
| `docs/xiangqiai-feature-analysis.md` | 新增：本文档 |
