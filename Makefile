# LumenOS 构建系统
# 支持 BIOS (MBR) 和 UEFI (GPT) 双启动模式
#
# 前置条件:
#   - Rust nightly (rust-toolchain.toml 已固定)
#   - rust-src + llvm-tools-preview (rust-toolchain.toml 已包含)
#   - QEMU (qemu-system-x86_64)
#   - OVMF (UEFI 固件，仅 UEFI 模式需要): sudo apt install ovmf

KERNEL_ELF    := target/x86_64-unknown-none/debug/lumenos
KERNEL_ELF_R  := target/x86_64-unknown-none/release/lumenos
BIOS_IMG      := target/x86_64-unknown-none/debug/lumenos-bios.img
UEFI_IMG      := target/x86_64-unknown-none/debug/lumenos-uefi.img
BIOS_IMG_R    := target/x86_64-unknown-none/release/lumenos-bios.img
UEFI_IMG_R    := target/x86_64-unknown-none/release/lumenos-uefi.img
HOST_TARGET   := $(shell rustc -vV | sed -n 's/host: //p')
BUILDER_BIN   := builder/target/$(HOST_TARGET)/release/lumenos-builder
OVMF_CODE     := /usr/share/OVMF/OVMF_CODE_4M.fd

.PHONY: build build-release builder bios-image uefi-image \
        run-bios run-uefi test clean help

help:
	@echo "LumenOS 构建命令:"
	@echo "  make build          编译内核 (debug)"
	@echo "  make build-release  编译内核 (release)"
	@echo "  make builder        编译磁盘镜像构建工具"
	@echo "  make bios-image     创建 BIOS (MBR) 启动磁盘镜像"
	@echo "  make uefi-image     创建 UEFI (GPT) 启动磁盘镜像"
	@echo "  make run-bios       以 BIOS 模式在 QEMU 中运行"
	@echo "  make run-uefi       以 UEFI 模式在 QEMU 中运行"
	@echo "  make test           运行所有测试"
	@echo "  make clean          清理构建产物"

# ── 编译 ──

build:
	cargo build

build-release:
	cargo build --release

builder:
	cd builder && cargo build --release --target $(HOST_TARGET)

# ── 磁盘镜像 ──

bios-image: build builder
	$(BUILDER_BIN) --image bios $(KERNEL_ELF) $(BIOS_IMG)

uefi-image: build builder
	$(BUILDER_BIN) --image uefi $(KERNEL_ELF) $(UEFI_IMG)

# ── 运行 ──

run-bios: bios-image
	qemu-system-x86_64 \
		-drive format=raw,file=$(BIOS_IMG) \
		-device isa-debug-exit,iobase=0xf4,iosize=0x04 \
		-serial stdio

run-uefi: uefi-image
	@if [ ! -f "$(OVMF_CODE)" ]; then \
		echo "错误: 未找到 OVMF 固件 ($(OVMF_CODE))"; \
		echo "请安装: sudo apt install ovmf"; \
		exit 1; \
	fi
	qemu-system-x86_64 \
		-drive if=pflash,format=raw,readonly=on,file=$(OVMF_CODE) \
		-drive format=raw,file=$(UEFI_IMG) \
		-device isa-debug-exit,iobase=0xf4,iosize=0x04 \
		-serial stdio

# ── 测试 ──

test: builder
	cargo test

# ── 清理 ──

clean:
	cargo clean
	cd builder && cargo clean 2>/dev/null || true
