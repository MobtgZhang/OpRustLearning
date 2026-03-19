# 可选快捷命令：仍需 nightly、rust-src、bootimage、QEMU

.PHONY: build image run

build:
	cargo build

image:
	cargo bootimage

run: image
	cargo run
