# blog_os（教学复刻）

与 [phil-opp / blog_os](https://github.com/phil-opp/blog_os) 及 [Writing an OS in Rust](https://os.phil-opp.com/) 第二版前若干篇一致的最小 **x86_64** 内核：`no_std`、BIOS 引导、`bootloader` + `bootimage`、VGA 文本模式与全局 `println!`。

本仓库中 `writing-an-os-in-rust/` 为参考译文，**不是**本 crate 的源码树；实现原则对齐该系列。

## 依赖环境

- Rust **nightly**（已用 `rust-toolchain.toml` 固定）
- `rust-src`、`llvm-tools-preview`（toolchain 里已列出）
- [QEMU](https://www.qemu.org/) `qemu-system-x86_64`（可选，用于运行）
- [bootimage](https://github.com/rust-osdev/bootimage)：`cargo install bootimage`（生成可引导 `.bin`）

## 构建与运行

```bash
cd blog_os
cargo build
cargo bootimage   # 生成 target/x86_64-unknown-none/debug/bootimage-blog_os.bin
cargo run         # 先构建镜像，再用 bootimage runner 启动 QEMU
```

手动 QEMU：

```bash
qemu-system-x86_64 -drive format=raw,file=target/x86_64-unknown-none/debug/bootimage-blog_os.bin
```

## 技术说明

- 使用 **内置目标** `x86_64-unknown-none` + Cargo `-Z build-std`，无需自定义 JSON 与 `cargo-xbuild`。
- VGA 缓冲区写入使用 `core::ptr::{read_volatile, write_volatile}`，避免被错误优化。
- `lazy_static` + `spin::Mutex` 提供可打印的全局 `WRITER`。

## 许可

教程代码通常为 MIT OR Apache-2.0 双许可；本目录实现按协议可同样采用（见 `Cargo.toml`）。
