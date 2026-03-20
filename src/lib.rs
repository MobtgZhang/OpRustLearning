//! # LumenOS 内核库
//!
//! 本模块是 LumenOS 的核心库，包含所有内核子系统的公共接口。
//!
//! ## 设计思路
//!
//! 参考 [Writing an OS in Rust](https://os.phil-opp.com/) 系列教程，
//! LumenOS 采用 `lib.rs` + `main.rs` 的分离架构：
//! - `lib.rs`（本文件）：定义所有公共模块和共享功能，可被集成测试引用
//! - `main.rs`：内核入口点，调用库中的初始化函数
//!
//! ## 启动方式
//!
//! 使用 `bootloader_api` v0.11，同时支持 **BIOS（MBR）** 和 **UEFI（GPT）** 启动：
//! - BIOS 启动：bootloader 通过传统 MBR 引导，设置 VBE 图形帧缓冲区
//! - UEFI 启动：bootloader 通过 UEFI 固件引导，使用 GOP 帧缓冲区
//! - 两种模式下内核代码完全相同，通过统一的 `BootInfo` 接口获取硬件信息
//!
//! ## 模块概览
//!
//! | 模块 | 功能 |
//! |------|------|
//! | [`framebuffer`] | 像素帧缓冲区文本渲染驱动 |
//! | [`serial`] | UART 串口输出（调试用） |
//! | [`gdt`] | 全局描述符表（GDT）与任务状态段（TSS） |
//! | [`interrupts`] | 中断描述符表（IDT）与中断处理程序 |
//! | [`memory`] | 内存分页与物理帧分配 |
//! | [`allocator`] | 堆内存分配器 |

#![no_std]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]

extern crate alloc;

pub mod allocator;
pub mod framebuffer;
pub mod gdt;
pub mod interrupts;
pub mod memory;
pub mod serial;

use bootloader_api::config::{BootloaderConfig, Mapping};
use core::panic::PanicInfo;

/// Bootloader 配置（编译期嵌入内核 ELF）
///
/// 通过 `entry_point!` 宏传递给 bootloader，控制其行为：
/// - `physical_memory = Some(Mapping::Dynamic)`: 将全部物理内存映射到虚拟地址空间
///   bootloader 会自动选择一个合适的虚拟地址偏移量，通过 BootInfo 传递给内核
pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

/// 初始化内核所有子系统
///
/// 调用顺序很重要：
/// 1. GDT — 需要在 IDT 之前加载，因为双重错误处理需要 IST 栈
/// 2. IDT — 注册 CPU 异常和硬件中断的处理函数
/// 3. PIC — 初始化 8259 可编程中断控制器，取消所有中断屏蔽
/// 4. 开启中断 — 执行 `sti` 指令，CPU 开始响应中断
pub fn init() {
    gdt::init();
    interrupts::init_idt();
    unsafe { interrupts::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();
}

/// 使用 HLT 指令进入低功耗循环
///
/// 与空的 `loop {}` 不同，`hlt` 会让 CPU 休眠直到下一个中断到来，
/// 从而大幅降低 CPU 占用率。这是裸机内核的标准主循环模式。
pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

// ============================================================================
// 自定义测试框架
// ============================================================================

/// 可测试对象的 trait
///
/// 所有实现了 `Fn()` 的类型自动实现此 trait，
/// 在运行测试时会打印完整的函数路径名并报告结果。
pub trait Testable {
    fn run(&self);
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

/// 自定义测试运行器：依次执行所有测试用例
pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

/// 测试中的 panic 处理：输出错误信息后退出 QEMU
pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    hlt_loop()
}

/// QEMU 退出码
///
/// 通过向 isa-debug-exit 设备的 I/O 端口（0xF4）写入值来控制 QEMU 退出。
/// QEMU 实际返回的退出码 = (写入值 << 1) | 1，
/// 所以 Success(0x10) 对应退出码 33，Failed(0x11) 对应退出码 35。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

/// 向 QEMU 的 isa-debug-exit 设备写入退出码，触发 QEMU 退出
pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;
    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

// ============================================================================
// 库模式下的测试入口（cargo test --lib 时使用）
// ============================================================================

#[cfg(test)]
use bootloader_api::{entry_point, BootInfo};

#[cfg(test)]
entry_point!(test_kernel_main, config = &BOOTLOADER_CONFIG);

#[cfg(test)]
fn test_kernel_main(_boot_info: &'static mut BootInfo) -> ! {
    init();
    test_main();
    hlt_loop()
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}
