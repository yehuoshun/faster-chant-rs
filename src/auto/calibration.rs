use crate::scheme::scheme::SchemeManager;
use anyhow::Result;
use log::info;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use windows::Win32::Foundation::HWND;

/// 校准状态
pub struct Calibration {
    /// 当前激活的方案 display_name
    pub active_scheme: Mutex<Option<String>>,
    /// 是否正在搜索中
    pub searching: AtomicBool,
}

impl Calibration {
    pub fn new() -> Self {
        Self {
            active_scheme: Mutex::new(None),
            searching: AtomicBool::new(false),
        }
    }

    /// 自动校准：OCR 确认页 → 匹配方案
    pub fn auto_calibrate(
        &self,
        skin_name: &str,
        hero_name: &str,
        schemes: &SchemeManager,
    ) -> Option<String> {
        let matched = schemes.match_scheme(skin_name, hero_name);
        let name = matched.map(|s| s.display_name.clone());

        if let Some(ref n) = name {
            *self.active_scheme.lock().unwrap() = Some(n.clone());
            info!("自动校准: {}", n);
        }
        name
    }

    /// 手动校准：用户输入拼音/关键词搜索
    pub fn search(&self, input: &str, schemes: &SchemeManager) -> Vec<String> {
        schemes.search_pinyin(input)
    }

    /// 手动选择方案
    pub fn select(&self, name: &str) {
        *self.active_scheme.lock().unwrap() = Some(name.to_string());
        info!("手动校准: {}", name);
    }

    /// 获取当前激活的方案名
    pub fn current(&self) -> Option<String> {
        self.active_scheme.lock().unwrap().clone()
    }

    /// 清除当前方案
    pub fn clear(&self) {
        *self.active_scheme.lock().unwrap() = None;
    }
}

/// 热键 ID
pub const HOTKEY_CALIBRATE: u32 = 1;

/// 注册全局热键 Ctrl+Shift+H
pub fn register_hotkey(hwnd: HWND) -> Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, MOD_CONTROL, MOD_SHIFT, VK_H,
    };
    unsafe {
        RegisterHotKey(
            hwnd,
            HOTKEY_CALIBRATE,
            MOD_CONTROL | MOD_SHIFT | windows::Win32::UI::Input::KeyboardAndMouse::MOD_NOREPEAT,
            VK_H.0 as u32,
        )?;
    }
    info!("热键注册: Ctrl+Shift+H → 校准");
    Ok(())
}

/// 注销热键
pub fn unregister_hotkey(hwnd: HWND) -> Result<()> {
    use windows::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey;
    unsafe {
        UnregisterHotKey(hwnd, HOTKEY_CALIBRATE)?;
    }
    Ok(())
}