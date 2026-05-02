<script setup lang="ts">
import { listen } from '@tauri-apps/api/event';
import { NCard, NFlex, NText, NScrollbar, NTag } from 'naive-ui';
import { ref, computed, nextTick } from 'vue';

interface AnalyseData {
    depth: number,
    score: number,
    time: number,
    nodes: number,
    nps: number,
    wdl: [number, number, number] | null,
    pvs: string[],
    moves: string[],
    state: string,
    source: string,
}

const current = ref<AnalyseData | null>(null);
const history = ref<AnalyseData[]>([]);
const historyContainer = ref<InstanceType<typeof NScrollbar> | null>(null);

const bestMove = computed(() => current.value?.moves?.[0] ?? '----');
const score = computed(() => current.value?.score ?? 0);
const depth = computed(() => current.value?.depth ?? 0);
const source = computed(() => current.value?.source ?? '');

const scoreText = computed(() => {
    const s = score.value;
    if (s >= 29990) return `杀 ${30000 - s}`;
    if (s <= -29990) return `杀 ${30000 + s}`;
    return (s / 100).toFixed(2);
});

const scoreColor = computed(() => {
    const s = score.value;
    if (s > 200) return '#c0392b';
    if (s < -200) return '#2c3e50';
    return '#7f8c8d';
});

const npsText = computed(() => {
    const n = current.value?.nps ?? 0;
    if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M';
    if (n >= 1_000) return (n / 1_000).toFixed(0) + 'K';
    return String(n);
});

const timeText = computed(() => {
    const ms = current.value?.time ?? 0;
    if (ms >= 1000) return (ms / 1000).toFixed(1) + 's';
    return ms + 'ms';
});

const nodesText = computed(() => {
    const n = current.value?.nodes ?? 0;
    if (n >= 1_000_000_000) return (n / 1_000_000_000).toFixed(1) + 'G';
    if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M';
    if (n >= 1_000) return (n / 1_000).toFixed(0) + 'K';
    return String(n);
});

const pvLine = computed(() => current.value?.moves?.join(' ') ?? '');

const wdl = computed(() => {
    if (!current.value?.wdl) return null;
    const [w, d, l] = current.value.wdl;
    const total = w + d + l || 1;
    return {
        win: (w / total * 100),
        draw: (d / total * 100),
        loss: (l / total * 100),
        winPct: (w / 10).toFixed(1),
        drawPct: (d / 10).toFixed(1),
        lossPct: (l / 10).toFixed(1),
    };
});

listen('analyse', async (event) => {
    const data = event.payload as AnalyseData;
    current.value = data;

    if (history.value.length > 0) {
        const last = history.value[history.value.length - 1];
        if (data.depth <= last.depth && data.source === last.source) {
            history.value = [];
        }
    }
    history.value.push(data);
    if (history.value.length > 200) {
        history.value.shift();
    }

    await nextTick();
    historyContainer.value?.scrollTo({ top: 99999 });
});
</script>

<template>
    <div class="analyse-panel">
        <n-card :bordered="false" class="analyse-card" content-style="padding: 8px 12px">
            <!-- 最佳走法 + 分数 -->
            <div class="best-section">
                <div class="best-move" :style="{ color: scoreColor }">
                    {{ bestMove }}
                </div>
                <div class="score-block">
                    <div class="score-value" :style="{ color: scoreColor }">
                        {{ scoreText }}
                    </div>
                    <n-tag v-if="source" size="tiny" :bordered="false" type="info">
                        {{ source }}
                    </n-tag>
                </div>
            </div>

            <!-- 统计信息 -->
            <n-flex class="stats-bar" size="small" :wrap="false">
                <div class="stat-item">
                    <span class="stat-label">深度</span>
                    <span class="stat-value">{{ depth }}</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">NPS</span>
                    <span class="stat-value">{{ npsText }}</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">节点</span>
                    <span class="stat-value">{{ nodesText }}</span>
                </div>
                <div class="stat-item">
                    <span class="stat-label">耗时</span>
                    <span class="stat-value">{{ timeText }}</span>
                </div>
            </n-flex>

            <!-- WDL 概率条 -->
            <div v-if="wdl" class="wdl-bar">
                <div class="wdl-win" :style="{ width: wdl.win + '%' }" :title="`胜 ${wdl.winPct}%`">
                    <span v-if="wdl.win > 12">{{ wdl.winPct }}%</span>
                </div>
                <div class="wdl-draw" :style="{ width: wdl.draw + '%' }" :title="`和 ${wdl.drawPct}%`">
                    <span v-if="wdl.draw > 12">{{ wdl.drawPct }}%</span>
                </div>
                <div class="wdl-loss" :style="{ width: wdl.loss + '%' }" :title="`负 ${wdl.lossPct}%`">
                    <span v-if="wdl.loss > 12">{{ wdl.lossPct }}%</span>
                </div>
            </div>

            <!-- PV 线 -->
            <div v-if="pvLine" class="pv-line">
                <n-text depth="3" style="font-size: 11px">{{ pvLine }}</n-text>
            </div>
        </n-card>

        <!-- 历史分析记录 -->
        <n-card :bordered="false" class="history-card" content-style="padding: 4px 8px">
            <n-scrollbar ref="historyContainer" style="max-height: 100%">
                <div class="history-list">
                    <div
                        v-for="(item, idx) in history"
                        :key="idx"
                        class="history-item"
                    >
                        <span class="hist-depth">D{{ item.depth }}</span>
                        <span class="hist-score" :style="{ color: item.score > 0 ? '#c0392b' : item.score < 0 ? '#2c3e50' : '#7f8c8d' }">
                            {{ (item.score / 100).toFixed(2) }}
                        </span>
                        <span class="hist-moves">{{ item.moves.join(' ') }}</span>
                    </div>
                </div>
            </n-scrollbar>
        </n-card>
    </div>
</template>

<style scoped>
.analyse-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    gap: 4px;
}

.analyse-card {
    flex-shrink: 0;
}

.history-card {
    flex: 1;
    min-height: 0;
    overflow: hidden;
}

.best-section {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 6px;
}

.best-move {
    font-size: 22px;
    font-weight: 700;
    letter-spacing: 2px;
}

.score-block {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
}

.score-value {
    font-size: 18px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
}

.stats-bar {
    background: #f5f7fa;
    border-radius: 4px;
    padding: 4px 8px;
    margin-bottom: 6px;
}

.stat-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    flex: 1;
}

.stat-label {
    font-size: 10px;
    color: #999;
}

.stat-value {
    font-size: 12px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: #333;
}

.wdl-bar {
    display: flex;
    height: 16px;
    border-radius: 3px;
    overflow: hidden;
    margin-bottom: 6px;
    font-size: 10px;
    font-weight: 500;
    color: #fff;
    line-height: 16px;
    text-align: center;
}

.wdl-win {
    background: #c0392b;
    transition: width 0.3s;
}

.wdl-draw {
    background: #95a5a6;
    transition: width 0.3s;
}

.wdl-loss {
    background: #2c3e50;
    transition: width 0.3s;
}

.pv-line {
    background: #f5f7fa;
    border-radius: 4px;
    padding: 4px 8px;
    word-break: break-all;
    line-height: 1.6;
}

.history-list {
    display: flex;
    flex-direction: column;
}

.history-item {
    display: flex;
    align-items: baseline;
    gap: 6px;
    padding: 1px 4px;
    font-size: 11px;
    border-bottom: 1px solid #f0f0f0;
    line-height: 1.8;
}

.hist-depth {
    flex-shrink: 0;
    width: 28px;
    color: #999;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
}

.hist-score {
    flex-shrink: 0;
    width: 44px;
    text-align: right;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
}

.hist-moves {
    color: #555;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}
</style>
