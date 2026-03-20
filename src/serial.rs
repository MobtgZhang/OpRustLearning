//! # UART 串口输出驱动
//!
//! ## 为什么需要串口？
//!
//! VGA 文本模式虽然能在屏幕上显示信息，但在自动化测试中不方便读取。
//! 串口（Serial Port）可以将数据直接传输到宿主机的标准输出，
//! 非常适合输出测试结果和调试信息。
//!
//! ## UART 16550
//!
//! UART (Universal Asynchronous Receiver/Transmitter) 是最常见的串口标准。
//! 16550 是 PC 上的经典 UART 芯片型号，有以下关键 I/O 端口：
//!
//! | 端口地址 | 用途 |
//! |----------|------|
//! | 0x3F8 | COM1 数据端口（读/写数据） |
//! | 0x3F9 | COM1 中断使能 |
//! | 0x3FA | COM1 FIFO 控制 |
//! | 0x3FB | COM1 线路控制（数据位、停止位、校验） |
//! | 0x3FD | COM1 线路状态（发送/接收就绪） |
//!
//! 我们使用 `uart_16550` crate 封装了这些底层细节。
//!
//! ## QEMU 集成
//!
//! QEMU 的 `-serial stdio` 参数会将虚拟机的串口输出重定向到宿主机的标准输出，
//! 测试框架就是通过这个机制将测试结果传递给宿主机的。

use lazy_static::lazy_static;
use spin::Mutex;
use uart_16550::SerialPort;

lazy_static! {
    /// 全局串口实例（COM1, I/O 端口 0x3F8）
    ///
    /// `init()` 会配置波特率、数据位（8 bit）、停止位（1 bit）等参数。
    /// 使用 Mutex 保护，确保多个调用者不会交错输出乱码。
    pub static ref SERIAL1: Mutex<SerialPort> = {
        let mut serial_port = unsafe { SerialPort::new(0x3F8) };
        serial_port.init();
        Mutex::new(serial_port)
    };
}

/// 串口打印的内部实现
///
/// 同样使用 `without_interrupts` 防止在持有锁时被中断打断导致死锁。
#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        SERIAL1
            .lock()
            .write_fmt(args)
            .expect("serial output failed");
    });
}

/// 向串口打印格式化文本（不换行）
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*))
    };
}

/// 向串口打印格式化文本（自动换行）
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}
