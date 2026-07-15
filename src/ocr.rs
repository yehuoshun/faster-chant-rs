use anyhow::{Context, Result};
use log::{debug, info};
use windows::{
    core::HSTRING,
    Globalization::Language,
    Graphics::Imaging::{
        BitmapAlphaMode, BitmapEncoder, BitmapPixelFormat, SoftwareBitmap,
    },
    Media::Ocr::{OcrEngine, OcrLine, OcrResult, OcrWord},
    Storage::Streams::{DataWriter, InMemoryRandomAccessStream},
};

use crate::config::AppConfig;
use crate::window;

/// OCR 引擎（惰性初始化，全局单例）
pub struct Ocr {
    engine: OcrEngine,
}

impl Ocr {
    /// 创建 OCR 引擎，使用系统默认语言（中文 Windows 会自动支持中文）
    pub fn new() -> Result<Self> {
        // 尝试中文语言，失败则用默认
        let engine = OcrEngine::TryCreateFromLanguage(&Language::CreateLanguage(&HSTRING::from("zh-Hans"))?)
            .or_else(|_| {
                debug!("中文 OCR 引擎不可用，尝试系统默认语言");
                OcrEngine::TryCreateFromUserProfileLanguages()
            })?;

        info!("OCR 引擎初始化成功，语言: {:?}", engine.RecognizerLanguage()?.DisplayName()?);
        Ok(Self { engine })
    }

    /// 从窗口区域截取并 OCR 识别文字
    /// 返回识别到的文本，多行用空格连接
    pub fn recognize_region(
        &self,
        hwnd: windows::Win32::Foundation::HWND,
        region: &crate::config::RegionRatio,
    ) -> Result<String> {
        let rect = window::get_window_rect(hwnd);
        let win_w = rect.right - rect.left;
        let win_h = rect.bottom - rect.top;

        let x = (win_w as f64 * region.x) as i32;
        let y = (win_h as f64 * region.y) as i32;
        let w = (win_w as f64 * region.w).max(1.0) as i32;
        let h = (win_h as f64 * region.h).max(1.0) as i32;

        debug!("OCR 识别区域: x={}, y={}, w={}, h={}", x, y, w, h);

        // 截取窗口像素
        let img = window::capture_window_region(hwnd, x, y, w, h)
            .context("截取窗口区域失败")?;

        // 转换为 SoftwareBitmap（BGRA8）
        let (img_w, img_h) = img.dimensions();
        let software_bitmap = SoftwareBitmap::CreateCopyFromBuffer(
            &img.into_raw(),
            BitmapPixelFormat::Bgra8,
            img_w as i32,
            img_h as i32,
            BitmapAlphaMode::Premultiplied,
        )?;

        // OCR 识别
        let result = self.engine
            .RecognizeAsync(&software_bitmap)?
            .get()?;

        let text = extract_text(&result);
        debug!("OCR 识别结果: {}", text);
        Ok(text)
    }

    /// 识别皮肤名区域，返回 (皮肤名, 英雄名)
    pub fn recognize_skin_name(
        &self,
        hwnd: windows::Win32::Foundation::HWND,
        cfg: &AppConfig,
    ) -> Result<(String, String)> {
        let text = self.recognize_region(hwnd, &cfg.skin_name_region)?;
        Ok(crate::scheme::parse_skin_text(&text))
    }
}

/// 从 OcrResult 中提取文本
fn extract_text(result: &OcrResult) -> String {
    let lines = result.Lines().unwrap_or_default();
    let mut texts: Vec<String> = Vec::new();

    for i in 0..lines.Size().unwrap_or(0) {
        if let Ok(line) = lines.GetAt(i) {
            let words = line.Words().unwrap_or_default();
            let mut line_text = String::new();
            for j in 0..words.Size().unwrap_or(0) {
                if let Ok(word) = words.GetAt(j) {
                    if let Ok(text) = word.Text() {
                        if !line_text.is_empty() {
                            line_text.push(' ');
                        }
                        line_text.push_str(&text.to_string_lossy());
                    }
                }
            }
            if !line_text.is_empty() {
                texts.push(line_text);
            }
        }
    }

    texts.join(" ")
}

impl Ocr {
    /// 识别指定图片区域中的文字（用于调试）
    pub fn recognize_image(&self, img: &image::RgbaImage) -> Result<String> {
        let (w, h) = img.dimensions();
        let software_bitmap = SoftwareBitmap::CreateCopyFromBuffer(
            &img.clone().into_raw(),
            BitmapPixelFormat::Bgra8,
            w as i32,
            h as i32,
            BitmapAlphaMode::Premultiplied,
        )?;

        let result = self.engine
            .RecognizeAsync(&software_bitmap)?
            .get()?;

        Ok(extract_text(&result))
    }
}