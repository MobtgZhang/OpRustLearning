//! # 帧缓冲区文本渲染驱动
//!
//! ## 概述
//!
//! bootloader v0.11 同时支持 BIOS 和 UEFI 启动。两种模式下都提供一个
//! **像素帧缓冲区**（framebuffer）用于屏幕显示，取代了传统的 VGA 文本模式。
//!
//! 帧缓冲区是一块连续的内存区域，每个像素占若干字节（通常 3 或 4 字节），
//! 直接对应屏幕上的像素点。要显示文字，需要自行将字符逐像素"绘制"到缓冲区中。
//!
//! ## 字体渲染
//!
//! 使用 `noto-sans-mono-bitmap` crate 提供的预光栅化等宽字体。
//! 每个字符已经被预处理为像素强度数组（0=透明，255=完全不透明），
//! 支持抗锯齿效果，比传统的 1-bit 位图字体更美观。
//!
//! ## 像素格式
//!
//! 不同硬件/固件使用不同的像素字节序：
//! - **RGB**: 红-绿-蓝顺序（常见于 UEFI）
//! - **BGR**: 蓝-绿-红顺序（常见于 BIOS/VBE）
//! - **U8**: 单字节灰度（少见）

use bootloader_api::info::{FrameBufferInfo, PixelFormat};
use core::fmt;
use noto_sans_mono_bitmap::{get_raster, get_raster_width, FontWeight, RasterHeight};
use spin::Mutex;

const FONT_WEIGHT: FontWeight = FontWeight::Regular;
const FONT_HEIGHT: RasterHeight = RasterHeight::Size16;
const LINE_SPACING: usize = 2;
const BORDER_PADDING: usize = 1;
const CHAR_RASTER_HEIGHT: usize = 16;
const CHAR_RASTER_WIDTH: usize = get_raster_width(FONT_WEIGHT, FONT_HEIGHT);
const CHAR_CELL_HEIGHT: usize = CHAR_RASTER_HEIGHT + LINE_SPACING;

/// 全局帧缓冲区写入器（初始为 None，在 kernel_main 中初始化）
pub static WRITER: Mutex<Option<FrameBufferWriter>> = Mutex::new(None);

/// 初始化全局帧缓冲区写入器
///
/// # Safety
///
/// `buffer` 必须是有效的帧缓冲区内存切片，且在程序运行期间始终有效。
pub fn init(buffer: &'static mut [u8], info: FrameBufferInfo) {
    let writer = FrameBufferWriter::new(buffer, info);
    *WRITER.lock() = Some(writer);
}

/// 帧缓冲区文本写入器
///
/// 在像素帧缓冲区上模拟文本终端行为：
/// - 维护文本光标位置（x_pos, y_pos 以像素为单位）
/// - 使用位图字体逐字符渲染
/// - 支持换行和自动滚屏
pub struct FrameBufferWriter {
    framebuffer: &'static mut [u8],
    info: FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
}

impl FrameBufferWriter {
    fn new(framebuffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        let mut writer = Self {
            framebuffer,
            info,
            x_pos: BORDER_PADDING,
            y_pos: BORDER_PADDING,
        };
        writer.clear();
        writer
    }

    /// 清空整个屏幕（填充黑色）并重置光标
    pub fn clear(&mut self) {
        self.x_pos = BORDER_PADDING;
        self.y_pos = BORDER_PADDING;
        self.framebuffer.fill(0);
    }

    fn width(&self) -> usize {
        self.info.width
    }

    fn height(&self) -> usize {
        self.info.height
    }

    fn newline(&mut self) {
        self.y_pos += CHAR_CELL_HEIGHT;
        self.carriage_return();
        if self.y_pos + CHAR_RASTER_HEIGHT >= self.height() {
            self.scroll();
        }
    }

    fn carriage_return(&mut self) {
        self.x_pos = BORDER_PADDING;
    }

    /// 滚屏：将全部内容上移一行（CHAR_CELL_HEIGHT 像素），清空底部区域
    fn scroll(&mut self) {
        let bytes_per_pixel = self.info.bytes_per_pixel;
        let stride = self.info.stride;
        let height = self.height();

        let row_bytes = stride * bytes_per_pixel;
        let src_start = CHAR_CELL_HEIGHT * row_bytes;
        let copy_len = (height.saturating_sub(CHAR_CELL_HEIGHT)) * row_bytes;

        if src_start + copy_len <= self.framebuffer.len() {
            self.framebuffer.copy_within(src_start..src_start + copy_len, 0);
        }

        let clear_start = copy_len;
        let clear_end = core::cmp::min(height * row_bytes, self.framebuffer.len());
        if clear_start < clear_end {
            self.framebuffer[clear_start..clear_end].fill(0);
        }

        self.y_pos = self.y_pos.saturating_sub(CHAR_CELL_HEIGHT);
    }

    /// 写入单个字符
    fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            c => {
                let new_x = self.x_pos + CHAR_RASTER_WIDTH;
                if new_x >= self.width() {
                    self.newline();
                }
                let rendered = get_raster(c, FONT_WEIGHT, FONT_HEIGHT)
                    .unwrap_or_else(|| get_raster('?', FONT_WEIGHT, FONT_HEIGHT).unwrap());
                self.write_rendered_char(rendered);
            }
        }
    }

    /// 将光栅化字符的像素数据写入帧缓冲区
    fn write_rendered_char(&mut self, rendered: noto_sans_mono_bitmap::RasterizedChar) {
        for (y, row) in rendered.raster().iter().enumerate() {
            for (x, &intensity) in row.iter().enumerate() {
                self.write_pixel(self.x_pos + x, self.y_pos + y, intensity);
            }
        }
        self.x_pos += rendered.width();
    }

    /// 在指定坐标写入一个像素
    ///
    /// `intensity` 为字体的不透明度（0=透明，255=完全不透明）。
    /// 使用绿色前景（致敬经典终端风格）。
    fn write_pixel(&mut self, x: usize, y: usize, intensity: u8) {
        if x >= self.width() || y >= self.height() {
            return;
        }

        let pixel_offset = y * self.info.stride + x;
        let color = match self.info.pixel_format {
            PixelFormat::Rgb => [0, intensity, 0, 0],
            PixelFormat::Bgr => [0, intensity, 0, 0],
            PixelFormat::U8 => [intensity, 0, 0, 0],
            other => {
                // 未知像素格式，回退为白色
                let _ = other;
                [intensity, intensity, intensity, 0]
            }
        };

        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;
        if byte_offset + bytes_per_pixel <= self.framebuffer.len() {
            self.framebuffer[byte_offset..(byte_offset + bytes_per_pixel)]
                .copy_from_slice(&color[..bytes_per_pixel]);
        }
    }
}

impl fmt::Write for FrameBufferWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}

// ============================================================================
// print! / println! 宏
// ============================================================================

/// 向帧缓冲区打印格式化文本（不换行）
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::framebuffer::_print(format_args!($($arg)*)));
}

/// 向帧缓冲区打印格式化文本（自动换行）
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

/// 打印函数的内部实现（由宏调用，不应直接使用）
///
/// 使用 `without_interrupts` 防止在持有 WRITER 锁时被中断打断导致死锁。
/// 如果帧缓冲区尚未初始化（WRITER 为 None），输出会被静默丢弃。
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        if let Some(writer) = WRITER.lock().as_mut() {
            writer.write_fmt(args).unwrap();
        }
    });
}
