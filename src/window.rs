use crate::config::AppConfig;
use anyhow::{Context, Result};
use log::debug;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{BitBlt, CreateCompatibleDC, CreateCompatibleBitmap, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject, DIB_RGB_COLORS, BITMAPINFO, BITMAPINFOHEADER, SRCCOPY};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetClientRect, GetWindowRect};

/// 查找游戏窗口，返回窗口句柄
pub fn find_game_window(title_keyword: &str) -> Option<HWND> {
    // 枚举所有顶层窗口，检查标题是否包含关键词
    // 简化实现：用 FindWindowW 尝试匹配
    // 实际 300 英雄窗口类名/标题需要实际测试确认
    let title = format!("{}\0", title_keyword);
    let hwnd = unsafe {
        // 先用类名匹配，再用标题匹配
        FindWindowW(None, PCWSTR::from_raw(
            title.encode_utf16().collect::<Vec<u16>>().as_ptr(),
        ))
        .ok()
        .or_else(|| {
            // 如果类名匹配失败，尝试其他方式
            None
        })
    };
    hwnd
}

/// 获取窗口客户区矩形（相对于桌面）
pub fn get_client_rect_screen(hwnd: HWND) -> Result<RECT> {
    unsafe {
        let mut rect = RECT::default();
        GetClientRect(hwnd, &mut rect)?;
        // 转换为屏幕坐标
        let mut pt = windows::Win32::Foundation::POINT { x: rect.left, y: rect.top };
        windows::Win32::UI::WindowsAndMessaging::ClientToScreen(hwnd, &mut pt)?;
        Ok(RECT {
            left: pt.x,
            top: pt.y,
            right: pt.x + (rect.right - rect.left),
            bottom: pt.y + (rect.bottom - rect.top),
        })
    }
}

/// 检测蓝色宝石锚点是否存在
pub fn check_blue_gem(hwnd: HWND, cfg: &AppConfig) -> bool {
    let client_rect = match get_client_rect_screen(hwnd) {
        Ok(r) => r,
        Err(e) => {
            debug!("获取窗口矩形失败: {}", e);
            return false;
        }
    };

    let width = (client_rect.right - client_rect.left) as u32;
    let height = (client_rect.bottom - client_rect.top) as u32;

    if width == 0 || height == 0 {
        return false;
    }

    // 计算宝石区域在窗口内的像素坐标
    let region = &cfg.gem_region;
    let x = (width as f64 * region.x) as i32;
    let y = (height as f64 * region.y) as i32;
    let w = (width as f64 * region.w).max(1.0) as i32;
    let h = (height as f64 * region.h).max(1.0) as i32;

    // 采样区域中心附近的几个像素
    let sample_count = 5;
    let mut matches = 0;

    match capture_pixels(hwnd, client_rect, x, y, w, h) {
        Ok(pixels) => {
            for pixel in &pixels {
                if is_in_range(pixel, &cfg.gem_color) {
                    matches += 1;
                }
            }
        }
        Err(e) => {
            debug!("截取像素失败: {}", e);
            return false;
        }
    }

    debug!("蓝色宝石检测: {}/{} 像素匹配", matches, sample_count);
    matches >= 3 // 至少 3 个像素在蓝色范围内
}

/// 从窗口截取像素数据
fn capture_pixels(
    hwnd: HWND,
    screen_rect: RECT,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Result<Vec<(u8, u8, u8)>> {
    unsafe {
        let screen_x = screen_rect.left + x;
        let screen_y = screen_rect.top + y;

        let screen_dc = GetDC(None);
        if screen_dc.is_invalid() {
            anyhow::bail!("GetDC 失败");
        }

        let mem_dc = CreateCompatibleDC(screen_dc);
        if mem_dc.is_invalid() {
            ReleaseDC(None, screen_dc);
            anyhow::bail!("CreateCompatibleDC 失败");
        }

        let bitmap = CreateCompatibleBitmap(screen_dc, w, h);
        if bitmap.is_invalid() {
            DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);
            anyhow::bail!("CreateCompatibleBitmap 失败");
        }

        let old_bmp = SelectObject(mem_dc, bitmap);
        BitBlt(mem_dc, 0, 0, w, h, screen_dc, screen_x, screen_y, SRCCOPY)?;

        // 获取像素数据
        let data_size = (w * h * 4) as usize;
        let mut data = vec![0u8; data_size];

        let mut bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // 负值 = 自上而下
                biPlanes: 1,
                biBitCount: 32,
                biCompression: 0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [Default::default(); 1],
        };

        GetDIBits(
            mem_dc,
            bitmap,
            0,
            h as u32,
            Some(data.as_mut_ptr() as *mut _),
            &mut bi,
            DIB_RGB_COLORS,
        )?;

        // 清理
        SelectObject(mem_dc, old_bmp);
        DeleteObject(bitmap);
        DeleteDC(mem_dc);
        ReleaseDC(None, screen_dc);

        // 采样中心区域几个像素
        let step = (w * h / 5).max(1) as usize;
        let mut pixels = Vec::new();
        for i in (0..data.len()).step_by(4).step_by(step) {
            if i + 3 < data.len() {
                pixels.push((data[i + 2], data[i + 1], data[i])); // BGR → RGB
                if pixels.len() >= 5 {
                    break;
                }
            }
        }
        Ok(pixels)
    }
}

/// 判断像素 RGB 是否在目标范围内
fn is_in_range(pixel: &(u8, u8, u8), target: &crate::config::ColorRange) -> bool {
    pixel.0 >= target.r_min
        && pixel.0 <= target.r_max
        && pixel.1 >= target.g_min
        && pixel.1 <= target.g_max
        && pixel.2 >= target.b_min
        && pixel.2 <= target.b_max
}

/// 获取窗口矩形（屏幕坐标）
pub fn get_window_rect(hwnd: HWND) -> Option<RECT> {
    unsafe {
        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect).ok()?;
        Some(rect)
    }
}