#![no_std] // 不链接Rust标准库
#![no_main] // 禁用所有Rust层级的入口点
use core::panic::PanicInfo; 

#[panic_handler] //这个函数将在发生panic的时候被调用它
fn panic(_info:&PanicInfo) -> ! {
    loop {}
}

#[no_mangle] // 不重复函数名
pub extern "C" fn _start() -> ! {
    //默认入口函数为 _start
    loop {}
}

