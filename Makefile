.PHONY: check check-node check-pnpm check-rust check-tauri check-libs \
       install dev build build-macos build-linux \
       build-win-cpu build-win-gpu build-win-cpu-rotate build-win-gpu-rotate \
       clean help

SHELL := /bin/bash
export PATH := $(HOME)/.cargo/bin:$(PATH)

GREEN  := \033[32m
YELLOW := \033[33m
RED    := \033[31m
CYAN   := \033[36m
RESET  := \033[0m

NODE_MIN_VERSION := 18
LIBS_DIR := libs
PIKAFISH_DIR := $(LIBS_DIR)/pikafish
BUNDLE_DIR := server/target/release/bundle

OS := $(shell uname -s)

# ========== 帮助 ==========

help: ## 显示帮助信息
	@printf "\n"
	@printf "$(CYAN)中国象棋学习助手 (xqlink) — 构建系统$(RESET)\n"
	@printf "\n"
	@printf "$(YELLOW)环境检查:$(RESET)\n"
	@printf "  make check              检查所有构建依赖\n"
	@printf "\n"
	@printf "$(YELLOW)开发:$(RESET)\n"
	@printf "  make install            安装前端依赖\n"
	@printf "  make dev                开发模式运行\n"
	@printf "\n"
	@printf "$(YELLOW)生产构建:$(RESET)\n"
	@printf "  make build              根据当前平台自动选择构建\n"
	@printf "  make build-macos        macOS 构建 (.dmg)\n"
	@printf "  make build-linux        Linux 构建 (.deb)\n"
	@printf "  make build-win-cpu      Windows CPU 构建 (.msi)\n"
	@printf "  make build-win-gpu      Windows GPU 构建 (.msi)\n"
	@printf "  make build-win-cpu-rotate  Windows CPU + 旋转模型构建\n"
	@printf "  make build-win-gpu-rotate  Windows GPU + 旋转模型构建\n"
	@printf "\n"
	@printf "$(YELLOW)其他:$(RESET)\n"
	@printf "  make clean              清理构建产物\n"
	@printf "  make help               显示此帮助信息\n"
	@printf "\n"

# ========== 环境检查 ==========

check-node:
	@printf "$(CYAN)检查 Node.js ...$(RESET) "
	@command -v node >/dev/null 2>&1 || { printf "$(RED)未安装 Node.js$(RESET)\n"; exit 1; }
	@NODE_VER=$$(node -v | sed 's/v//' | cut -d. -f1); \
	if [ "$$NODE_VER" -lt $(NODE_MIN_VERSION) ]; then \
		printf "$(RED)版本过低: v$$NODE_VER (需要 >= $(NODE_MIN_VERSION))$(RESET)\n"; exit 1; \
	fi
	@printf "$(GREEN)✓ %s$(RESET)\n" "$$(node -v)"

check-pnpm:
	@printf "$(CYAN)检查 pnpm ...$(RESET) "
	@command -v pnpm >/dev/null 2>&1 || { printf "$(RED)未安装 pnpm (npm install -g pnpm)$(RESET)\n"; exit 1; }
	@printf "$(GREEN)✓ %s$(RESET)\n" "$$(pnpm -v)"

check-rust:
	@printf "$(CYAN)检查 Rust 工具链 ...$(RESET) "
	@command -v rustc >/dev/null 2>&1 || { printf "$(RED)未安装 Rust (https://rustup.rs)$(RESET)\n"; exit 1; }
	@command -v cargo >/dev/null 2>&1 || { printf "$(RED)未找到 cargo$(RESET)\n"; exit 1; }
	@printf "$(GREEN)✓ %s$(RESET)\n" "$$(rustc --version)"

check-tauri:
	@printf "$(CYAN)检查 Tauri CLI ...$(RESET) "
	@(pnpm tauri --version >/dev/null 2>&1) || { printf "$(RED)不可用，请先运行 make install$(RESET)\n"; exit 1; }
	@printf "$(GREEN)✓ %s$(RESET)\n" "$$(pnpm tauri --version 2>/dev/null)"

check-libs:
	@printf "$(CYAN)检查外部资源 (libs/) ...$(RESET)\n"
	@HAS_MODEL=0; \
	if [ -f "$(LIBS_DIR)/large.onnx" ]; then HAS_MODEL=1; printf "  $(GREEN)✓ large.onnx$(RESET)\n"; \
	else printf "  $(YELLOW)✗ large.onnx 不存在$(RESET)\n"; fi; \
	if [ -f "$(LIBS_DIR)/rotate.onnx" ]; then HAS_MODEL=1; printf "  $(GREEN)✓ rotate.onnx$(RESET)\n"; \
	else printf "  $(YELLOW)✗ rotate.onnx 不存在$(RESET)\n"; fi; \
	if [ "$$HAS_MODEL" -eq 0 ]; then printf "  $(RED)✗ 缺少 ONNX 模型 (需要 large.onnx 或 rotate.onnx)$(RESET)\n"; exit 1; fi
	@if [ -f "$(PIKAFISH_DIR)/pikafish.nnue" ]; then \
		printf "  $(GREEN)✓ pikafish.nnue$(RESET)\n"; \
	else \
		printf "  $(RED)✗ pikafish.nnue 不存在$(RESET)\n"; exit 1; \
	fi
	@case "$(OS)" in \
		Darwin) \
			if [ -f "$(PIKAFISH_DIR)/pikafish-macos" ]; then \
				printf "  $(GREEN)✓ pikafish-macos$(RESET)\n"; \
			else \
				printf "  $(RED)✗ pikafish-macos 不存在$(RESET)\n"; exit 1; \
			fi;; \
		Linux) \
			if [ -f "$(PIKAFISH_DIR)/pikafish-linux" ]; then \
				printf "  $(GREEN)✓ pikafish-linux$(RESET)\n"; \
			else \
				printf "  $(RED)✗ pikafish-linux 不存在$(RESET)\n"; exit 1; \
			fi;; \
		*) \
			if [ -f "$(PIKAFISH_DIR)/pikafish-windows.exe" ]; then \
				printf "  $(GREEN)✓ pikafish-windows.exe$(RESET)\n"; \
			else \
				printf "  $(RED)✗ pikafish-windows.exe 不存在$(RESET)\n"; exit 1; \
			fi;; \
	esac

check: check-node check-pnpm check-rust check-tauri check-libs ## 检查所有构建依赖
	@printf "\n$(GREEN)所有依赖检查通过！$(RESET)\n"

# ========== 安装 ==========

install: check-node check-pnpm ## 安装前端依赖
	pnpm install

# ========== 开发 ==========

dev: check-node check-pnpm check-rust check-libs ## 开发模式运行
	pnpm tauri dev

# ========== 构建 ==========

build: check ## 根据当前平台自动构建
	@case "$(OS)" in \
		Darwin) $(MAKE) build-macos;; \
		Linux)  $(MAKE) build-linux;; \
		*)      printf "$(YELLOW)Windows 请手动选择 build-win-cpu 或 build-win-gpu$(RESET)\n"; exit 1;; \
	esac

build-macos: check ## macOS 构建
	@printf "\n$(CYAN)开始 macOS 构建 ...$(RESET)\n"
	pnpm tauri build --config server/tauri.macos.conf.json
	@printf "\n$(GREEN)构建完成！产物位于 $(BUNDLE_DIR)/$(RESET)\n"

build-linux: check ## Linux 构建
	@printf "\n$(CYAN)开始 Linux 构建 ...$(RESET)\n"
	pnpm tauri build --config server/tauri.linux.conf.json
	@printf "\n$(GREEN)构建完成！产物位于 $(BUNDLE_DIR)/$(RESET)\n"

build-win-cpu: check ## Windows CPU 构建
	@printf "\n$(CYAN)开始 Windows (CPU) 构建 ...$(RESET)\n"
	pnpm run build:cpu
	@printf "\n$(GREEN)构建完成！产物位于 $(BUNDLE_DIR)/$(RESET)\n"

build-win-gpu: check ## Windows GPU 构建
	@printf "\n$(CYAN)开始 Windows (GPU) 构建 ...$(RESET)\n"
	pnpm run build:gpu
	@printf "\n$(GREEN)构建完成！产物位于 $(BUNDLE_DIR)/$(RESET)\n"

build-win-cpu-rotate: check ## Windows CPU + 旋转模型构建
	@printf "\n$(CYAN)开始 Windows (CPU + Rotate) 构建 ...$(RESET)\n"
	pnpm run build:cpu:rotate
	@printf "\n$(GREEN)构建完成！产物位于 $(BUNDLE_DIR)/$(RESET)\n"

build-win-gpu-rotate: check ## Windows GPU + 旋转模型构建
	@printf "\n$(CYAN)开始 Windows (GPU + Rotate) 构建 ...$(RESET)\n"
	pnpm run build:gpu:rotate
	@printf "\n$(GREEN)构建完成！产物位于 $(BUNDLE_DIR)/$(RESET)\n"

# ========== 清理 ==========

clean: ## 清理构建产物
	@printf "$(CYAN)清理前端构建 ...$(RESET)\n"
	rm -rf dist
	@printf "$(CYAN)清理 Rust 构建 ...$(RESET)\n"
	cd server && cargo clean
	@printf "$(GREEN)清理完成$(RESET)\n"
