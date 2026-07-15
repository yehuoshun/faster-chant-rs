use anyhow::Result;
use log::debug;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, KEYEVENTF_KEYUP, VK_CONTROL, VK_RETURN, VK_SHIFT, VK_V,
};

/// 游戏聊天框最大字符数（中文约 50-60 字）
const MAX_CHAT_LEN: usize = 50;

/// 发送消息，超长自动拆分多条
pub fn send_message(text: &str) -> Result<()> {
    for chunk in split_text(text, MAX_CHAT_LEN) {
        send_single(&chunk)?;
    }
    Ok(())
}

/// 发送到全体频道
pub fn send_all_chat(text: &str) -> Result<()> {
    for chunk in split_text(text, MAX_CHAT_LEN) {
        send_all_single(&chunk)?;
    }
    Ok(())
}

/// 拆分长文本
fn split_text(text: &str, max_len: usize) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let end = (start + max_len).min(chars.len());
        // 尽量在标点处断句
        let mut split_at = end;
        if split_at < chars.len() {
            for i in (start..split_at).rev() {
                if "，。！？；：、,.!?;:".contains(chars[i]) {
                    split_at = i + 1;
                    break;
                }
            }
        }
        chunks.push(chars[start..split_at].iter().collect());
        start = split_at;
    }
    chunks
}

/// 发送单条消息
fn send_single(text: &str) -> Result<()> {
    debug!("发送消息: {}", text);

    // 1. 复制到剪贴板
    set_clipboard(text)?;

    // 2. 模拟按键：Enter → Ctrl+V → Enter
    unsafe {
        // 打开聊天框
        keybd_event(VK_RETURN.0 as u8, 0, 0, 0);
        keybd_event(VK_RETURN.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        std::thread::sleep(std::time::Duration::from_millis(50));

        // 粘贴
        keybd_event(VK_CONTROL.0 as u8, 0, 0, 0);
        keybd_event(VK_V.0 as u8, 0, 0, 0);
        keybd_event(VK_V.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        std::thread::sleep(std::time::Duration::from_millis(30));

        // 发送
        keybd_event(VK_RETURN.0 as u8, 0, 0, 0);
        keybd_event(VK_RETURN.0 as u8, 0, KEYEVENTF_KEYUP, 0);
    }

    Ok(())
}

/// 发送单条全体消息
fn send_all_single(text: &str) -> Result<()> {
    debug!("发送全体消息: {}", text);

    set_clipboard(text)?;

    unsafe {
        // Enter → 打开聊天框
        keybd_event(VK_RETURN.0 as u8, 0, 0, 0);
        keybd_event(VK_RETURN.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Shift+Enter → 切换到全体频道
        keybd_event(VK_SHIFT.0 as u8, 0, 0, 0);
        keybd_event(VK_RETURN.0 as u8, 0, 0, 0);
        keybd_event(VK_RETURN.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_SHIFT.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        std::thread::sleep(std::time::Duration::from_millis(30));

        // Ctrl+V → 粘贴
        keybd_event(VK_CONTROL.0 as u8, 0, 0, 0);
        keybd_event(VK_V.0 as u8, 0, 0, 0);
        keybd_event(VK_V.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        std::thread::sleep(std::time::Duration::from_millis(30));

        // Enter → 发送
        keybd_event(VK_RETURN.0 as u8, 0, 0, 0);
        keybd_event(VK_RETURN.0 as u8, 0, KEYEVENTF_KEYUP, 0);
    }

    Ok(())
}

/// 设置剪贴板文本
fn set_clipboard(text: &str) -> Result<()> {
    use windows::Win32::System::Ole::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData, CF_UNICODETEXT,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    unsafe {
        if !OpenClipboard(None).as_bool() {
            anyhow::bail!("无法打开剪贴板");
        }
        EmptyClipboard()?;

        // 分配全局内存
        let text_wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let mem_size = text_wide.len() * 2;
        let hmem = GlobalAlloc(GMEM_MOVEABLE, mem_size)?;

        let ptr = GlobalLock(hmem);
        std::ptr::copy_nonoverlapping(
            text_wide.as_ptr(),
            ptr as *mut u16,
            text_wide.len(),
        );
        GlobalUnlock(hmem);

        SetClipboardData(CF_UNICODETEXT.0 as u32, hmem)?;
        CloseClipboard()?;
    }

    Ok(())
}

/// 从台词列表中随机选一条
pub fn pick_random(lines: &[String]) -> Option<&str> {
    if lines.is_empty() {
        return None;
    }
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let idx = (seed as usize) % lines.len();
    Some(&lines[idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pick_random() {
        let lines = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let result = pick_random(&lines);
        assert!(result.is_some());
        assert!(lines.contains(&result.unwrap().to_string()));
    }

    #[test]
    fn test_pick_random_empty() {
        assert!(pick_random(&[]).is_none());
    }

    #[test]
    fn test_split_text_short() {
        let result = split_text("你好", 50);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "你好");
    }

    #[test]
    fn test_split_text_long() {
        let text = "  ".repeat(60);
        let result = split_text(&text, 50);
        assert!(result.len() >= 2);
        // 每段不超过 50
        for chunk in &result {
            assert!(chunk.chars().count() <= 50);
        }
    }
}