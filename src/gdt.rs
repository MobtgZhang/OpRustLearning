//! # 全局描述符表 (GDT) 与任务状态段 (TSS)
//!
//! ## 什么是 GDT？
//!
//! GDT (Global Descriptor Table) 是 x86 架构从实模式时代遗留下来的结构。
//! 在 16/32 位时代，GDT 用于定义内存段的基地址、长度和访问权限（分段机制）。
//! 在 64 位长模式下，分段功能已被分页完全取代，但 GDT 仍然不可或缺：
//!
//! 1. **内核态/用户态切换** — CPU 通过代码段选择子（CS 寄存器）区分特权级
//! 2. **加载 TSS** — TSS 描述符只能存放在 GDT 中
//!
//! ## 什么是 TSS？
//!
//! TSS (Task State Segment) 在 32 位时代用于硬件任务切换。
//! 在 64 位模式下，硬件任务切换已废弃，TSS 的主要用途变成了保存
//! **中断栈表 (IST, Interrupt Stack Table)**。
//!
//! IST 包含 7 个栈指针（IST[0]~IST[6]），在 IDT 条目中可以指定
//! 使用哪个 IST 栈。当对应的中断/异常发生时，CPU 会自动切换到 IST 指定的栈。
//!
//! ## 段寄存器重载
//!
//! 加载新的 GDT 后，必须重新加载段寄存器。bootloader v0.11 在 BIOS 和 UEFI
//! 模式下设置的段寄存器可能指向旧 GDT 的条目。加载新 GDT 后需要：
//! - CS: 使用新 GDT 的内核代码段选择子
//! - SS: 在 64 位模式下设为空选择子（ring 0 允许）
//! - TSS: 使用新 GDT 的 TSS 段选择子

use lazy_static::lazy_static;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

/// 双重错误处理使用的 IST 索引
///
/// 我们使用 IST[0]（第一个槽位），在 IDT 的双重错误条目中会引用这个索引。
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    /// 全局 TSS 实例
    ///
    /// 配置 IST[0] 为一块 20KB 的独立栈空间，专用于双重错误处理。
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5; // 20 KiB
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(&raw const STACK);
            stack_start + STACK_SIZE
        };
        tss
    };
}

lazy_static! {
    /// 全局 GDT 及其段选择子
    ///
    /// GDT 包含两个描述符条目：
    /// 1. **内核代码段** — CPU 在 Ring 0（内核态）执行时使用
    /// 2. **TSS 段** — 指向上面定义的 TSS 结构体
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        (
            gdt,
            Selectors {
                code_selector,
                tss_selector,
            },
        )
    };
}

struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

/// 加载 GDT 并更新相关 CPU 寄存器
///
/// 加载新 GDT 后，必须重新设置段寄存器：
/// - CS: 使用 `set_reg` 远跳转到新的代码段
/// - SS: 设为空选择子（64 位 ring 0 允许）— **UEFI 兼容必需**
/// - TSS: 加载新的 TSS 选择子，使 CPU 知道 IST 栈的位置
pub fn init() {
    use x86_64::instructions::segmentation::{Segment, CS, SS};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        SS::set_reg(SegmentSelector(0));
        load_tss(GDT.1.tss_selector);
    }
}
