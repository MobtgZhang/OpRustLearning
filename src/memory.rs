//! # 内存管理：分页与物理帧分配
//!
//! ## x86_64 的四级页表
//!
//! x86_64 使用 **四级页表** 将 48 位虚拟地址翻译为物理地址：
//!
//! ```text
//! 虚拟地址 (48 位有效，高 16 位为符号扩展)
//! ┌────────┬────────┬────────┬────────┬──────────────┐
//! │ L4 索引 │ L3 索引 │ L2 索引 │ L1 索引 │  页内偏移      │
//! │ (9 bit) │ (9 bit) │ (9 bit) │ (9 bit) │  (12 bit)    │
//! └────────┴────────┴────────┴────────┴──────────────┘
//!
//! 翻译过程:
//!   CR3 寄存器 → L4 页表 → L3 页表 → L2 页表 → L1 页表 → 物理帧起始地址
//!   最终物理地址 = 物理帧起始地址 + 页内偏移
//! ```
//!
//! ## bootloader 的物理内存映射策略
//!
//! 在 `BootloaderConfig` 中设置 `physical_memory = Some(Mapping::Dynamic)`，
//! bootloader 会将 **全部物理内存** 线性映射到一个虚拟地址区间：
//!
//! ```text
//! 物理地址:  0x0000_0000 ─────── 物理内存末尾
//!               │                      │
//!               ▼                      ▼
//! 虚拟地址:  offset + 0x0 ───── offset + 物理内存末尾
//! ```
//!
//! ## 帧分配器
//!
//! `BootInfoFrameAllocator` 从 bootloader 提供的内存区域表中
//! 找出所有标记为"可用"的物理内存区域，逐帧线性分配。
//! 适用于 BIOS 和 UEFI 两种启动模式。

use bootloader_api::info::{MemoryRegion, MemoryRegionKind};
use x86_64::structures::paging::{FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

/// 初始化页表映射器
///
/// 返回 `OffsetPageTable`——它封装了四级页表的遍历和修改操作，
/// 并且知道物理地址到虚拟地址的偏移关系。
///
/// # Safety
///
/// 调用者必须保证：
/// - `physical_memory_offset` 是 bootloader 映射物理内存的起始虚拟地址
/// - 全部物理内存确实已被映射到该偏移处
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

/// 获取当前活动的四级页表 (PML4) 的可变引用
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr
}

// ============================================================================
// 物理帧分配器
// ============================================================================

/// 基于 bootloader 内存区域表的物理帧分配器
///
/// 适用于 BIOS 和 UEFI 两种启动模式。bootloader_api v0.11 统一使用
/// `MemoryRegion` 和 `MemoryRegionKind` 来描述内存布局，
/// 与具体的启动方式无关。
pub struct BootInfoFrameAllocator {
    memory_regions: &'static [MemoryRegion],
    next: usize,
}

impl BootInfoFrameAllocator {
    /// 从 bootloader 的内存区域表创建帧分配器
    ///
    /// # Safety
    ///
    /// 调用者必须保证传入的 memory_regions 是有效的，
    /// 且所有标记为 `Usable` 的区域确实是空闲可用的物理内存。
    pub unsafe fn init(memory_regions: &'static [MemoryRegion]) -> Self {
        BootInfoFrameAllocator {
            memory_regions,
            next: 0,
        }
    }

    /// 构建所有可用物理帧的迭代器
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        let regions = self.memory_regions.iter();
        let usable_regions = regions.filter(|r| r.kind == MemoryRegionKind::Usable);
        let addr_ranges = usable_regions.map(|r| r.start..r.end);
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}
