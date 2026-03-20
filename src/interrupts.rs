//! # 中断与异常处理
//!
//! ## 中断概述
//!
//! CPU 通过 **中断描述符表 (IDT, Interrupt Descriptor Table)** 查找
//! 每种中断/异常对应的处理函数。IDT 最多 256 个条目（向量号 0~255）。
//!
//! 中断分为两大类：
//!
//! ### 1. CPU 异常（同步，由 CPU 执行指令时产生）
//!
//! | 向量号 | 名称 | 说明 |
//! |--------|------|------|
//! | 0 | 除零错误 | 除以零或除法溢出 |
//! | 3 | 断点 (BP) | 执行 INT3 指令 |
//! | 8 | 双重错误 (DF) | 处理异常时又触发异常 |
//! | 14 | 页错误 (PF) | 访问未映射/无权限的内存页 |
//!
//! ### 2. 硬件中断（异步，由外部设备通过 PIC 产生）
//!
//! | IRQ | 向量号 | 设备 |
//! |-----|--------|------|
//! | 0 | 32 | PIT 可编程间隔定时器 |
//! | 1 | 33 | PS/2 键盘 |
//!
//! ## 8259 PIC (可编程中断控制器)
//!
//! PC 使用两片级联的 8259 PIC 芯片管理 15 条硬件中断线：
//! - 主 PIC (Master): IRQ 0-7，重映射到中断向量 32-39
//! - 从 PIC (Slave): IRQ 8-15，重映射到中断向量 40-47
//!
//! 为什么要重映射？因为 PIC 默认将 IRQ 0-7 映射到向量 0-7，
//! 与 CPU 异常的向量号冲突。重映射到 32 以上就能避免冲突。
//!
//! ## x86-interrupt 调用约定
//!
//! 中断处理函数使用特殊的 `extern "x86-interrupt"` 调用约定，
//! 编译器会自动生成保存/恢复所有寄存器的代码，并使用 `iretq` 指令返回。

use crate::gdt;
use crate::{print, println};
use lazy_static::lazy_static;
use pic8259::ChainedPics;
use spin;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

/// 主 PIC 的中断向量偏移量（IRQ0 映射到向量 32）
pub const PIC_1_OFFSET: u8 = 32;
/// 从 PIC 的中断向量偏移量（IRQ8 映射到向量 40）
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

/// 全局 PIC 实例（两片 8259 级联）
///
/// 使用 `spin::Mutex` 保护，因为：
/// - 初始化时需要可变访问
/// - 每次中断处理结束后需要发送 EOI (End of Interrupt) 信号
pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

/// 硬件中断编号枚举
///
/// 每个 IRQ 对应的中断向量号 = PIC 偏移量 + IRQ 编号
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,     // IRQ0 → 向量 32
    Keyboard,                  // IRQ1 → 向量 33
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }
    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

// ============================================================================
// IDT 初始化
// ============================================================================

lazy_static! {
    /// 全局中断描述符表
    ///
    /// 为每种异常/中断注册对应的处理函数。
    /// 注意双重错误处理特别设置了 IST 索引，使用独立栈。
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        // ── CPU 异常处理 ──
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt.page_fault.set_handler_fn(page_fault_handler);

        // ── 硬件中断处理 ──
        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler);

        idt
    };
}

/// 加载 IDT 到 CPU
///
/// 执行 `lidt` 指令，将 IDT 的地址和大小写入 IDTR 寄存器。
/// 之后每当中断/异常发生，CPU 就会查找这个表来确定调用哪个处理函数。
pub fn init_idt() {
    IDT.load();
}

// ============================================================================
// CPU 异常处理函数
// ============================================================================

/// 断点异常处理 (#BP, 向量号 3)
///
/// 当 CPU 执行 `INT3` 指令（操作码 0xCC，单字节）时触发。
/// 调试器设置断点的原理就是将目标地址的第一个字节临时替换为 0xCC。
///
/// 这是一个"陷阱"(trap)类异常——处理完后，CPU 从触发指令的**下一条**继续执行。
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("=== CPU Exception: Breakpoint (#BP) ===");
    println!("{:#?}", stack_frame);
}

/// 双重错误处理 (#DF, 向量号 8)
///
/// 触发条件：处理某个异常时又触发了 CPU 无法处理的新异常。
/// 最常见的场景是栈溢出导致的级联错误。
///
/// 双重错误是终止类 (abort) 异常——无法恢复执行。
/// 如果双重错误处理本身也失败，就会触发三重错误 (Triple Fault)，
/// 此时 CPU 会直接执行硬件重启，没有任何软件层面的补救机会。
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("=== CPU Exception: Double Fault (#DF) ===\n{:#?}", stack_frame);
}

/// 页错误处理 (#PF, 向量号 14)
///
/// 触发条件：
/// - 访问未映射到物理内存的虚拟地址
/// - 访问权限不足（如用户态写入只读页、执行不可执行页）
/// - 页表条目的保留位被设置
///
/// CR2 寄存器自动保存触发错误的虚拟地址，error_code 包含错误的具体原因：
/// - bit 0 (P): 0=页不存在, 1=权限违规
/// - bit 1 (W): 0=读操作, 1=写操作触发
/// - bit 2 (U): 0=内核态, 1=用户态触发
extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("=== CPU Exception: Page Fault (#PF) ===");
    println!("Accessed virtual address: {:?}", Cr2::read());
    println!("Error code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    crate::hlt_loop();
}

// ============================================================================
// 硬件中断处理函数
// ============================================================================

/// 定时器中断处理 (IRQ0, 向量 32)
///
/// PIT (Programmable Interval Timer, 8253/8254 芯片) 以约 18.2 Hz 的默认频率
/// 周期性地触发此中断。在完整的操作系统中，定时器中断用于：
/// - 时间片轮转进程调度
/// - 维护系统时钟（jiffies / tick 计数器）
/// - 实现 sleep/timeout 等延时功能
///
/// 关键规则：每次处理完硬件中断后，**必须**向 PIC 发送 EOI (End of Interrupt)
/// 信号。否则 PIC 会认为上一个中断还没处理完，不会继续发送新的中断。
extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

/// 键盘中断处理 (IRQ1, 向量 33)
///
/// PS/2 键盘控制器在每次按键（按下或释放）时产生中断。
///
/// 处理流程：
/// 1. 从 I/O 端口 0x60 读取扫描码（scan code）
///    - 0x60 是 PS/2 控制器的数据端口
///    - 必须读取，否则控制器不会发送下一个按键中断
/// 2. 将扫描码交给 `pc-keyboard` crate 解码
///    - 单个按键可能产生多个扫描码（如扩展键前缀 0xE0）
///    - 解码器维护状态机来正确拼装多字节序列
/// 3. 如果解码得到可打印字符，输出到 VGA 屏幕
extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    use x86_64::instructions::port::Port;

    lazy_static! {
        /// 键盘解码器实例
        ///
        /// - `layouts::Us104Key`: 美式 104 键标准布局
        /// - `ScancodeSet1`: IBM PC/AT 兼容的扫描码集（PS/2 默认）
        /// - `HandleControl::Ignore`: 不将 Ctrl+字母 映射为控制字符
        static ref KEYBOARD: spin::Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
            spin::Mutex::new(Keyboard::new(
                ScancodeSet1::new(),
                layouts::Us104Key,
                HandleControl::Ignore,
            ));
    }

    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => print!("{}", character),
                DecodedKey::RawKey(key) => print!("{:?}", key),
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[test_case]
fn test_breakpoint_exception() {
    // 主动触发断点异常，验证处理函数能正常返回而不 panic
    x86_64::instructions::interrupts::int3();
}
