<script setup lang="ts">
import { NDialogProvider } from "naive-ui";
import Analyse from "./components/Analyse.vue";
import Chessboard from "./components/Chessboard.vue";
import Toolbar from "./components/Toolbar.vue";
</script>

<template>
    <n-dialog-provider>
        <div class="app-layout">
            <Toolbar class="app-toolbar" />
            <div class="app-main">
                <Chessboard class="app-board" />
                <Analyse class="app-analyse" />
            </div>
            <div class="app-footer"></div>
        </div>
    </n-dialog-provider>
</template>

<style>
html, body, #app {
    margin: 0;
    padding: 0;
    width: 100%;
    height: 100%;
    overflow: hidden;
}

.app-layout {
    display: flex;
    flex-direction: column;
    width: 100%;
    height: 100vh;
    overflow: hidden;
}

.app-toolbar {
    flex-shrink: 0;
}

.app-main {
    flex: 1;
    display: flex;
    min-height: 0;
    padding: 4px 8px;
    gap: 8px;
}

.app-board {
    flex-shrink: 0;
}

.app-analyse {
    flex: 1;
    min-width: 200px;
    overflow: auto;
}

.app-footer {
    flex-shrink: 0;
    padding: 2px 12px;
    font-size: x-small;
}
</style>
