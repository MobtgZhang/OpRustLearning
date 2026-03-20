//! # 栈溢出测试
//!
//! 验证栈溢出能被双重错误处理程序正确捕获，而不是导致三重错误重启。
//!
//! 测试策略：
//! 1. 初始化 GDT（包含 IST 栈）和自定义 IDT
//! 2. 执行无限递归函数，触发栈溢出
//! 3. 栈溢出 → 页错误 → 双重错误
//! 4. 双重错误处理函数使用 IST[0] 的独立栈，成功运行并输出 [ok]
//!
//! 此测试不使用标准测试框架（harness = false），因为我们需要完全控制执行流程。

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use bootloader_api::{entry_point, BootInfo};
use core::panic::PanicInfo;
use lazy_static::lazy_static;
use lumenos::{exit_qemu, serial_print, serial_println, QemuExitCode, BOOTLOADER_CONFIG};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

entry_point!(test_main_entry, config = &BOOTLOADER_CONFIG);

fn test_main_entry(_boot_info: &'static mut BootInfo) -> ! {
    serial_print!("stack_overflow::stack_overflow...\t");

    lumenos::gdt::init();
    init_test_idt();

    stack_overflow();

    panic!("Execution continued after stack overflow -- this should not happen");
}

#[allow(unconditional_recursion)]
fn stack_overflow() {
    stack_overflow();
    unsafe {
        core::ptr::read_volatile(&0 as *const i32);
    }
}

lazy_static! {
    static ref TEST_IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        unsafe {
            idt.double_fault
                .set_handler_fn(test_double_fault_handler)
                .set_stack_index(lumenos::gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt
    };
}

fn init_test_idt() {
    TEST_IDT.load();
}

extern "x86-interrupt" fn test_double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    lumenos::test_panic_handler(info)
}
