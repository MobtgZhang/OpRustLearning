#![no_std] // 不链接标准库
#![no_main] // 禁止所有Rust层级的入口点
use core::panic::PanicInfo;

// 这个函数将在panic时被调用
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

//static HELLO_STR: &[u8] = b'Hello World!';
#[no_mangle]
pub extern "C" fn _start() -> ! {
    /*let vga_buffer = 0xb8000 as *mut u8;

    for (i,&byte) in HELLO_STR.iter().enumerate(){
        unsafe {
            *vga_buffer.offset(i as isize*2) = byte;
            *vga_buffer.offset(i as isize*2+1) = 0xb;
        }
    }*/
    loop {}
}