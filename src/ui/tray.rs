use anyhow::Result;
use log::info;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetMessageW, InsertMenuItemW, PostQuitMessage, RegisterClassW, SetForegroundWindow,
    TrackPopupMenu, MENUITEMINFOW, MFS_DEFAULT, MIIM_ID, MIIM_STRING, TPM_BOTTOMALIGN,
    TPM_LEFTALIGN, WNDCLASSW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, WM_CREATE, WM_DESTROY,
    WM_LBUTTONUP, WM_RBUTTONUP, WS_OVERLAPPEDWINDOW,
};

const WM_TRAYICON: u32 = 0x0400 + 1;
const ID_TRAY_QUIT: u32 = 1001;
const ID_TRAY_EDITOR: u32 = 1002;
const ID_TRAY_CALIBRATE: u32 = 1003;

/// 托盘命令
#[derive(Debug, Clone)]
pub enum TrayCommand {
    Calibrate,
    OpenEditor,
    Quit,
}

/// 系统托盘
pub struct Tray {
    hwnd: HWND,
    running: Arc<AtomicBool>,
}

impl Tray {
    /// 创建托盘图标，返回命令接收通道
    pub fn spawn(running: Arc<AtomicBool>) -> Result<std::sync::mpsc::Receiver<TrayCommand>> {
        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::Builder::new()
            .name("tray".into())
            .spawn(move || {
                if let Err(e) = tray_thread(running, tx) {
                    log::error!("托盘线程异常: {}", e);
                }
            })?;

        Ok(rx)
    }
}

fn tray_thread(
    running: Arc<AtomicBool>,
    tx: std::sync::mpsc::Sender<TrayCommand>,
) -> Result<()> {
    unsafe {
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap();
        let class_name = windows::core::w!("FasterChantTray");

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(tray_wndproc),
            hInstance: hinstance,
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            windows::core::w!(""),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            hinstance,
            Some(&tx as *const _ as _),
        );

        if hwnd.0 == 0 {
            anyhow::bail!("托盘窗口创建失败");
        }

        // 添加托盘图标
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_MESSAGE | NIF_TIP | NIF_ICON,
            uCallbackMessage: WM_TRAYICON,
            hIcon: load_default_icon()?,
            ..Default::default()
        };
        let tip: Vec<u16> = "300高速咏唱\0".encode_utf16().collect();
        nid.szTip[..tip.len()].copy_from_slice(&tip);

        Shell_NotifyIconW(NIM_ADD, &nid)?;
        // 使用 v4 通知（支持现代 Windows）
        nid.uVersion = NOTIFYICON_VERSION_4;
        Shell_NotifyIconW(NIM_MODIFY, &nid).ok();

        info!("托盘图标已创建");

        // 消息循环
        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        while running.load(Ordering::Relaxed) {
            let ret = GetMessageW(&mut msg, hwnd, 0, 0);
            if ret.0 == 0 || ret.0 == -1 {
                break;
            }
            DispatchMessageW(&msg);
        }

        // 清理
        Shell_NotifyIconW(NIM_DELETE, &nid).ok();
        DestroyWindow(hwnd).ok();
        info!("托盘已退出");
    }
    Ok(())
}

unsafe extern "system" fn tray_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let cs = &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
            let tx_ptr = cs.lpCreateParams as *const std::sync::mpsc::Sender<TrayCommand>;
            windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::GWL_USERDATA,
                tx_ptr as isize,
            );
            LRESULT(0)
        }
        WM_TRAYICON => {
            if lparam.0 as u32 == WM_RBUTTONUP || lparam.0 as u32 == WM_LBUTTONUP {
                show_tray_menu(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn show_tray_menu(hwnd: HWND) {
    let menu = CreatePopupMenu().unwrap_or_default();

    // 校准
    let mut mii = std::mem::zeroed::<MENUITEMINFOW>();
    mii.cbSize = std::mem::size_of::<MENUITEMINFOW>() as u32;
    mii.fMask = MIIM_ID | MIIM_STRING;
    mii.wID = ID_TRAY_CALIBRATE;
    let text: Vec<u16> = "切换英雄\0".encode_utf16().collect();
    mii.dwTypeData = PCWSTR(text.as_ptr());
    InsertMenuItemW(menu, 0, true, &mii);

    // 编辑器
    mii.wID = ID_TRAY_EDITOR;
    let text: Vec<u16> = "英雄编辑器\0".encode_utf16().collect();
    mii.dwTypeData = PCWSTR(text.as_ptr());
    InsertMenuItemW(menu, 1, true, &mii);

    // 分隔线
    mii.fMask = MIIM_ID;
    mii.wID = 0;
    InsertMenuItemW(menu, 2, true, &mii);

    // 退出
    mii.fMask = MIIM_ID | MIIM_STRING;
    mii.wID = ID_TRAY_QUIT;
    let text: Vec<u16> = "退出\0".encode_utf16().collect();
    mii.dwTypeData = PCWSTR(text.as_ptr());
    InsertMenuItemW(menu, 3, true, &mii);

    // 显示菜单
    SetForegroundWindow(hwnd);
    let mut cursor_pos = windows::Win32::Foundation::POINT::default();
    windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut cursor_pos);

    let cmd = TrackPopupMenu(
        menu,
        TPM_LEFTALIGN | TPM_BOTTOMALIGN,
        cursor_pos.x,
        cursor_pos.y,
        0,
        hwnd,
        None,
    );

    windows::Win32::UI::WindowsAndMessaging::DestroyMenu(menu);

    // 发送命令
    let tx_ptr = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
        hwnd,
        windows::Win32::UI::WindowsAndMessaging::GWL_USERDATA,
    ) as *const std::sync::mpsc::Sender<TrayCommand>;
    let tx = &*tx_ptr;

    match cmd.0 as u32 {
        ID_TRAY_CALIBRATE => {
            let _ = tx.send(TrayCommand::Calibrate);
        }
        ID_TRAY_EDITOR => {
            let _ = tx.send(TrayCommand::OpenEditor);
        }
        ID_TRAY_QUIT => {
            let _ = tx.send(TrayCommand::Quit);
        }
        _ => {}
    }
}

/// 加载默认图标
fn load_default_icon() -> Result<windows::Win32::UI::WindowsAndMessaging::HICON> {
    unsafe {
        let icon = windows::Win32::UI::WindowsAndMessaging::LoadIconW(
            None,
            windows::Win32::UI::WindowsAndMessaging::IDI_APPLICATION,
        )?;
        Ok(icon)
    }
}