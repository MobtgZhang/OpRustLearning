#![no_std]
#![no_main]

mod vga_buffer;

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}!", "!");
    println!(
        "OpRustLearning — minimal kernel (phil-opp blog_os)，crate version {}",
        env!("CARGO_PKG_VERSION")
    );

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}
