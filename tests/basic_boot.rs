//! # 基础引导测试
//!
//! 验证内核能正常引导，并且基本的输出功能正常工作。
//! 这是最简单的集成测试——只要能成功引导并运行到 test_main，就算通过。
//! 测试在 BIOS 和 UEFI 两种模式下均可运行。

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(lumenos::test_runner)]
#![reexport_test_harness_main = "test_main"]

use bootloader_api::{entry_point, BootInfo};
use core::panic::PanicInfo;
use lumenos::{serial_println, BOOTLOADER_CONFIG};

entry_point!(test_main_entry, config = &BOOTLOADER_CONFIG);

fn test_main_entry(_boot_info: &'static mut BootInfo) -> ! {
    test_main();
    lumenos::hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    lumenos::test_panic_handler(info)
}

#[test_case]
fn test_println() {
    serial_println!("basic_boot::test_println -- basic boot test passed");
}
