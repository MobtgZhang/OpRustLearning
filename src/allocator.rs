//! # 堆内存分配器
//!
//! ## 为什么需要堆？
//!
//! 到目前为止，内核只能使用两种内存：
//! - **栈内存**：函数的局部变量，大小在编译期确定，函数返回后自动释放
//! - **静态内存**：全局变量和 `static` 常量，生命周期与程序相同
//!
//! 但很多场景需要 **运行时动态决定大小** 的内存：
//! - `Vec<T>`: 元素数量在运行时变化
//! - `Box<T>`: 将值移到堆上，通过指针间接访问
//! - `String`: 运行时构造的字符串
//! - `BTreeMap<K,V>`: 动态增长的映射表
//!
//! 这些类型都定义在 Rust 的 `alloc` 标准库中，
//! 它们通过全局分配器（`#[global_allocator]`）向操作系统申请/归还内存。
//!
//! ## 堆的实现步骤
//!
//! ```text
//! 1. 选定虚拟地址区间     → HEAP_START ~ HEAP_START + HEAP_SIZE
//! 2. 分配物理帧           → 从 BootInfoFrameAllocator 获取
//! 3. 建立页表映射         → 虚拟页 → 物理帧（可读+可写）
//! 4. 初始化分配器         → 告诉分配器堆的起始地址和大小
//! 5. 注册全局分配器       → #[global_allocator]
//! ```
//!
//! ## 分配器选择
//!
//! 本项目使用 `linked_list_allocator` crate 的 `LockedHeap`。
//! 它将空闲内存组织为一个**空闲链表**（free list）：
//!
//! ```text
//! ┌──────────┐    ┌──────────┐    ┌──────────┐
//! │ 空闲块 1  │───→│ 空闲块 2  │───→│ 空闲块 3  │───→ NULL
//! │ size: 64  │    │ size: 128 │    │ size: 256 │
//! └──────────┘    └──────────┘    └──────────┘
//! ```
//!
//! - **分配**: 遍历链表找到足够大的空闲块，拆分并返回
//! - **释放**: 将归还的内存块插回链表，尝试合并相邻空闲块
//!
//! 优点：实现简单，支持任意大小的分配/释放
//! 缺点：分配是 O(n)，容易产生外部碎片

use linked_list_allocator::LockedHeap;
use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB};
use x86_64::VirtAddr;

/// 堆的起始虚拟地址
///
/// 选择 `0x_4444_4444_0000` 是因为：
/// - 不与内核代码段、栈、bootloader 映射的区域冲突
/// - 地址模式明显（调试时一眼就能识别出是堆地址）
/// - 在 48 位规范虚拟地址空间的低半部分（用户空间范围）
pub const HEAP_START: usize = 0x_4444_4444_0000;

/// 堆的大小：100 KiB
///
/// 对于微内核的演示足够使用。
/// 实际操作系统会根据需要动态扩展堆（如 Linux 的 brk/mmap）。
pub const HEAP_SIZE: usize = 100 * 1024;

/// 全局堆分配器
///
/// `LockedHeap` = 自旋锁保护的链表分配器。
/// `#[global_allocator]` 属性告诉 Rust：所有通过 `alloc` crate 的内存请求
/// （Box::new、Vec::push 等）都转发给这个分配器处理。
///
/// 初始状态为空，必须在 `init_heap` 中完成初始化后才能使用。
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// 初始化堆内存
///
/// 执行以下操作：
/// 1. 计算堆区域需要覆盖的虚拟页范围
/// 2. 为每一页从帧分配器获取一个空闲物理帧
/// 3. 在页表中建立映射：虚拟页 → 物理帧（标记为 PRESENT + WRITABLE）
/// 4. 刷新 TLB 缓存（map_to 返回的 MapperFlush 自动处理）
/// 5. 用堆的起始地址和大小初始化链表分配器
///
/// 完成后，`alloc` crate 中的类型（Box、Vec、String 等）就可以正常使用了。
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}
