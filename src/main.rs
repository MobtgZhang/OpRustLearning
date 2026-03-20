//! # LumenOS 内核入口
//!
//! 这是操作系统内核的主入口文件。引导加载程序（bootloader）完成硬件初始化后，
//! 会跳转到这里定义的 `kernel_main` 函数开始执行内核代码。
//!
//! ## 启动流程（BIOS 和 UEFI 通用）
//!
//! ```text
//! 固件启动（BIOS POST 或 UEFI 初始化）
//!     │
//!     ▼
//! Bootloader（bootloader crate v0.11 提供，支持 BIOS/UEFI 双模式）
//!     │  ● 检测物理内存布局
//!     │  ● 设置四级页表（恒等映射 + 物理内存完整映射）
//!     │  ● 切换 CPU 到 64 位长模式（BIOS）/ 保持长模式（UEFI）
//!     │  ● 初始化像素帧缓冲区（VBE/GOP）
//!     │  ● 加载内核 ELF 到内存
//!     ▼
//! kernel_main(boot_info)    ← 我们的代码从这里开始
//!     │
//!     ├─ 初始化帧缓冲区显示
//!     ├─ 初始化 GDT / IDT / PIC
//!     ├─ 初始化页表映射器 + 帧分配器
//!     ├─ 初始化堆内存
//!     └─ 进入 hlt_loop 等待中断
//! ```

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(lumenos::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use bootloader_api::{entry_point, BootInfo};
use bootloader_api::info::Optional;
use core::panic::PanicInfo;
use lumenos::{print, println, serial_println};

entry_point!(kernel_main, config = &lumenos::BOOTLOADER_CONFIG);

/// 内核主函数 — 操作系统的真正起点
///
/// `boot_info` 由 bootloader 传入，包含物理内存映射表、帧缓冲区和物理内存偏移等关键信息。
/// 在 BIOS 和 UEFI 两种启动模式下，此函数接收到的 `BootInfo` 格式完全一致。
fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // ── 第一步：初始化帧缓冲区（屏幕输出） ──
    // bootloader 提供的像素帧缓冲区适用于 BIOS（VBE）和 UEFI（GOP）
    if let Optional::Some(framebuffer) = &mut boot_info.framebuffer {
        let info = framebuffer.info();
        let buf = framebuffer.buffer_mut();
        // Safety: 帧缓冲区由 bootloader 映射，内存在程序运行期间始终有效
        let buf: &'static mut [u8] = unsafe {
            core::slice::from_raw_parts_mut(buf.as_mut_ptr(), buf.len())
        };
        lumenos::framebuffer::init(buf, info);
    }

    // ── 第二步：在屏幕上打印欢迎横幅 ──
    print_banner();

    // ── 第三步：初始化内核子系统 ──
    lumenos::init();
    println!("[ok] GDT / IDT / PIC initialized");
    serial_println!("[ok] GDT / IDT / PIC initialized");

    // ── 第四步：初始化内存子系统 ──
    // bootloader 的 physical_memory 映射将全部物理内存映射到了
    // 从 physical_memory_offset 开始的虚拟地址空间。
    use lumenos::memory;
    use x86_64::VirtAddr;

    let phys_mem_offset = match boot_info.physical_memory_offset {
        Optional::Some(offset) => VirtAddr::new(offset),
        Optional::None => panic!("bootloader did not provide physical_memory_offset"),
    };
    let mut mapper = unsafe { memory::init(phys_mem_offset) };

    // 从 BootInfo 获取内存区域的 'static 引用
    let memory_regions = {
        let regions: &[bootloader_api::info::MemoryRegion] = &boot_info.memory_regions;
        unsafe { core::slice::from_raw_parts(regions.as_ptr(), regions.len()) }
    };
    let mut frame_allocator = unsafe { memory::BootInfoFrameAllocator::init(memory_regions) };
    println!("[ok] Page table mapper and frame allocator initialized");
    serial_println!("[ok] Page table mapper and frame allocator initialized");

    // ── 第五步：初始化堆内存 ──
    lumenos::allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("heap initialization failed");
    println!(
        "[ok] Heap initialized ({}KB @ 0x{:X})",
        lumenos::allocator::HEAP_SIZE / 1024,
        lumenos::allocator::HEAP_START
    );
    serial_println!(
        "[ok] Heap initialized ({}KB @ 0x{:X})",
        lumenos::allocator::HEAP_SIZE / 1024,
        lumenos::allocator::HEAP_START
    );

    // ── 第六步：演示堆分配功能 ──
    demo_heap_allocation();

    println!();
    println!("All subsystems initialized!");
    println!("Keyboard input will be echoed to screen, try typing :)");
    println!("Press Ctrl+C or close the QEMU window to exit.");

    #[cfg(test)]
    test_main();

    lumenos::hlt_loop();
}

/// 打印 ASCII 艺术启动横幅
fn print_banner() {
    println!();
    println!("  _                                ___  ____  ");
    println!(" | |   _   _ _ __ ___   ___ _ __  / _ \\/ ___| ");
    println!(" | |  | | | | '_ ` _ \\ / _ \\ '_ \\| | | \\___ \\ ");
    println!(" | |__| |_| | | | | | |  __/ | | | |_| |___) |");
    println!(" |_____\\__,_|_| |_| |_|\\___|_| |_|\\___/|____/ ");
    println!();
    println!(" LumenOS v{} - x86_64 Learning Microkernel", env!("CARGO_PKG_VERSION"));
    println!(" Supports BIOS (MBR) and UEFI (GPT) dual boot modes");
    println!(" Inspired by phil-opp/blog_os & Writing an OS in Rust");
    print!("------------------------------------------------------");
    println!();
    println!();
}

/// 演示堆分配功能：验证 Box 和 Vec 能正常工作
fn demo_heap_allocation() {
    let heap_value = Box::new(42);
    println!(
        "  Heap demo | Box<i32> value = {}, addr = {:p}",
        heap_value, &*heap_value
    );

    let mut numbers = Vec::new();
    for i in 0..10 {
        numbers.push(i * i);
    }
    println!("  Heap demo | Vec squares = {:?}", numbers);
}

// ============================================================================
// Panic 处理
// ============================================================================

/// 非测试模式：将 panic 信息打印到屏幕和串口，然后挂起
#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!();
    println!("!!! KERNEL PANIC !!!");
    println!("{}", info);
    serial_println!();
    serial_println!("!!! KERNEL PANIC !!!");
    serial_println!("{}", info);
    lumenos::hlt_loop()
}

/// 测试模式：通过串口输出 panic 信息，然后退出 QEMU
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    lumenos::test_panic_handler(info)
}
