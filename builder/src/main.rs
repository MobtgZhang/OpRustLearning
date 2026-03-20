//! LumenOS 磁盘镜像构建工具 & QEMU 运行器
//!
//! 两种模式：
//! 1. Runner 模式（cargo run / cargo test 自动调用）：
//!    lumenos-builder <kernel-binary>
//!    → 创建 BIOS 磁盘镜像，在 QEMU 中运行，转换退出码
//!
//! 2. 镜像创建模式：
//!    lumenos-builder --image <bios|uefi> <kernel-binary> <output-path>
//!    → 创建指定类型的磁盘镜像

use std::path::PathBuf;
use std::process::{Command, exit};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!("  {} <kernel-binary>                              (runner mode)", args[0]);
        eprintln!("  {} --image <bios|uefi> <kernel-binary> <output> (image mode)", args[0]);
        exit(1);
    }

    if args[1] == "--image" {
        image_mode(&args);
    } else {
        runner_mode(&args);
    }
}

fn image_mode(args: &[String]) {
    if args.len() < 5 {
        eprintln!("Usage: {} --image <bios|uefi> <kernel-binary> <output>", args[0]);
        exit(1);
    }

    let mode = &args[2];
    let kernel_path = PathBuf::from(&args[3]);
    let output_path = PathBuf::from(&args[4]);

    match mode.as_str() {
        "bios" => {
            bootloader::BiosBoot::new(&kernel_path)
                .create_disk_image(&output_path)
                .expect("failed to create BIOS disk image");
            println!("BIOS image created: {}", output_path.display());
        }
        "uefi" => {
            bootloader::UefiBoot::new(&kernel_path)
                .create_disk_image(&output_path)
                .expect("failed to create UEFI disk image");
            println!("UEFI image created: {}", output_path.display());
        }
        _ => {
            eprintln!("Unknown mode: {}, please use 'bios' or 'uefi'", mode);
            exit(1);
        }
    }
}

fn runner_mode(args: &[String]) {
    let kernel_path = PathBuf::from(&args[1]);

    let img_path = kernel_path.with_extension("img");

    bootloader::BiosBoot::new(&kernel_path)
        .create_disk_image(&img_path)
        .expect("failed to create BIOS disk image");

    let is_test = kernel_path
        .to_str()
        .map(|s| s.contains("/deps/"))
        .unwrap_or(false);

    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.arg("-drive")
        .arg(format!("format=raw,file={}", img_path.display()))
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04")
        .arg("-serial")
        .arg("stdio");

    if is_test {
        cmd.arg("-display").arg("none");
    }

    let status = cmd.status().expect("failed to start QEMU");
    let code = status.code().unwrap_or(1);

    // QEMU isa-debug-exit: success = (0x10 << 1) | 1 = 33, failed = (0x11 << 1) | 1 = 35
    match code {
        33 => exit(0),
        35 => exit(1),
        other => exit(other),
    }
}
