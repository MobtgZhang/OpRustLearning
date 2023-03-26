#![no_std] // 不链接标准库
#![no_main] // 禁止所有Rust层级的入口点
use core::panic::PanicInfo;

// 这个函数将在panic时被调用
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}