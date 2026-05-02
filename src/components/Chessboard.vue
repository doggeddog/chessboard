<script setup lang="ts">

import { listen } from "@tauri-apps/api/event";
import { onMounted, onUnmounted, ref } from "vue";

declare const vschess: any;

interface Position {
    piece: string;
    pos: string;
}

interface Changed {
    piece: string;
    from: string;
    to: string;
    camp: string;
}

const BOARD_W = 380;
const BOARD_H = 420;

const board = ref<string[][]>(
    Array.from({ length: 10 }, () => Array(9).fill(" "))
);

const scale = ref(1);
const scaledW = ref(BOARD_W);
const scaledH = ref(BOARD_H);
const container = ref<HTMLElement | null>(null);
let chess: any = null;
let resizeObserver: ResizeObserver | null = null;

function posToIndex(pos: string): [number, number] {
    const col = pos.charCodeAt(0) - "a".charCodeAt(0);
    const row = parseInt(pos[1]);
    return [row, col];
}

function boardToFen(): string {
    const rows: string[] = [];
    for (let row = 9; row >= 0; row--) {
        let fenRow = "";
        let emptyCount = 0;
        for (let col = 0; col < 9; col++) {
            const piece = board.value[row][col];
            if (piece === " ") {
                emptyCount++;
            } else {
                if (emptyCount > 0) {
                    fenRow += emptyCount;
                    emptyCount = 0;
                }
                fenRow += piece;
            }
        }
        if (emptyCount > 0) {
            fenRow += emptyCount;
        }
        rows.push(fenRow);
    }
    return rows.join("/") + " w - - 0 1";
}

function syncToVschess() {
    if (!chess) return;
    const fen = boardToFen();
    chess.setNode({ fen: fen, comment: "", next: [], defaultIndex: 0 });
    chess.rebuildSituation();
    chess.setBoardByStep(0);
}

function updateScale() {
    if (!container.value) return;
    const h = container.value.clientHeight;
    const s = h / BOARD_H;
    scale.value = Math.max(Math.min(s, 1.5), 0.5);
    scaledW.value = Math.round(BOARD_W * scale.value);
    scaledH.value = Math.round(BOARD_H * scale.value);
}

onMounted(() => {
    chess = new vschess.load("#vschess-board", {
        clickResponse: 0,
        sound: false,
        moveTips: false,
        saveTips: false,
    });

    updateScale();
    resizeObserver = new ResizeObserver(updateScale);
    if (container.value) resizeObserver.observe(container.value);

    listen("position", async (event) => {
        const positions = event.payload as Position[];
        for (let r = 0; r < 10; r++)
            for (let c = 0; c < 9; c++) board.value[r][c] = " ";
        for (const p of positions) {
            if (p.piece !== " ") {
                const [row, col] = posToIndex(p.pos);
                board.value[row][col] = p.piece;
            }
        }
        syncToVschess();
    });

    listen("move", async (event) => {
        const change = event.payload as Changed;
        const [fromRow, fromCol] = posToIndex(change.from);
        const [toRow, toCol] = posToIndex(change.to);
        board.value[fromRow][fromCol] = " ";
        board.value[toRow][toCol] = change.piece;
        syncToVschess();
    });

    listen("mirror", async (event) => {
        const isMirror = event.payload as boolean;
        if (chess) {
            chess.setTurn(isMirror ? 2 : 0);
        }
    });
});

onUnmounted(() => {
    resizeObserver?.disconnect();
});
</script>

<template>
    <div ref="container" class="board-container">
        <div
            class="board-sizer"
            :style="{
                width: scaledW + 'px',
                height: scaledH + 'px',
            }"
        >
            <div
                class="board-scaler"
                :style="{ transform: `scale(${scale})` }"
            >
                <div id="vschess-board"></div>
            </div>
        </div>
    </div>
</template>

<style>
.board-container {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
}

.board-sizer {
    position: relative;
    flex-shrink: 0;
    overflow: hidden;
    border-radius: 4px;
}

.board-scaler {
    position: absolute;
    top: 0;
    left: 0;
    transform-origin: top left;
}

/* vschess overrides */
.vschess-loaded {
    width: 380px !important;
    height: 420px !important;
    padding: 0 !important;
    border: none !important;
}

.vschess-loaded > *:not(.vschess-board) {
    display: none !important;
}

.vschess-board {
    top: 0 !important;
    left: 0 !important;
}
</style>
