use crate::scheme::SchemeManager;
use anyhow::Result;
use log::info;
use std::sync::{Arc, Mutex};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, DeleteObject, FillRect, GetStockObject, SelectObject, SetBkMode,
    SetTextColor, DEFAULT_GUI_FONT, RGB, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetCursorPos, GetMessageW,
    InvalidateRect, PostMessageW, RegisterClassW, SetWindowPos, ShowWindow,
    TranslateMessage, DispatchMessageW, WNDCLASSW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT,
    HWND_TOPMOST, SW_HIDE, SW_SHOW, WM_CHAR, WM_CREATE, WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN,
    WM_KILLFOCUS, WM_LBUTTONDOWN, WM_NCHITTEST, WM_PAINT, WM_SETFOCUS, WS_EX_TOPMOST,
    WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE, HTCAPTION, HTCLIENT, VK_DOWN, VK_ESCAPE,
    VK_RETURN, VK_UP,
};
use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;

const WINDOW_WIDTH: i32 = 320;
const WINDOW_HEIGHT: i32 = 400;
const PADDING: i32 = 12;
const INPUT_HEIGHT: i32 = 32;
const ITEM_HEIGHT: i32 = 28;
const FONT_SIZE: i32 = 16;

/// 搜索结果回调
pub type SearchCallback = Box<dyn Fn(&str) + Send + Sync>;

/// 搜索弹窗
pub struct SearchPopup {
    hwnd: HWND,
    input: Arc<Mutex<String>>,
    results: Arc<Mutex<Vec<String>>>,
    selected: Arc<Mutex<usize>>,
    schemes: Arc<SchemeManager>,
    on_select: Arc<Mutex<Option<SearchCallback>>>,
}

impl SearchPopup {
    pub fn new(schemes: Arc<SchemeManager>) -> Result<Self> {
        let input = Arc::new(Mutex::new(String::new()));
        let results = Arc::new(Mutex::new(Vec::new()));
        let selected = Arc::new(Mutex::new(0usize));
        let on_select: Arc<Mutex<Option<SearchCallback>>> =
            Arc::new(Mutex::new(None));

        let hwnd = create_window(&input, &results, &selected, &on_select, schemes.clone())?;

        Ok(Self {
            hwnd,
            input,
            results,
            selected,
            schemes,
            on_select,
        })
    }

    /// 显示搜索弹窗，返回用户选择的方案名
    pub fn show(&self) {
        // 重置状态
        *self.input.lock().unwrap() = String::new();
        *self.results.lock().unwrap() = self.schemes.search_pinyin("");
        *self.selected.lock().unwrap() = 0;

        // 居中显示
        let screen_w = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CXSCREEN,
            )
        };
        let screen_h = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(
                windows::Win32::UI::WindowsAndMessaging::SM_CYSCREEN,
            )
        };
        let x = (screen_w - WINDOW_WIDTH) / 2;
        let y = (screen_h - WINDOW_HEIGHT) / 2;

        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                x,
                y,
                WINDOW_WIDTH,
                WINDOW_HEIGHT,
                windows::Win32::UI::WindowsAndMessaging::SWP_SHOWWINDOW,
            );
            ShowWindow(self.hwnd, SW_SHOW);
            InvalidateRect(self.hwnd, None, true);
        }
    }

    pub fn hide(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn set_callback<F: Fn(&str) + Send + Sync + 'static>(&self, f: F) {
        *self.on_select.lock().unwrap() = Some(Box::new(f));
    }
}

impl Drop for SearchPopup {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.hwnd).ok();
        }
    }
}

fn create_window(
    input: &Arc<Mutex<String>>,
    results: &Arc<Mutex<Vec<String>>>,
    selected: &Arc<Mutex<usize>>,
    on_select: &Arc<Mutex<Option<SearchCallback>>>,
    schemes: Arc<SchemeManager>,
) -> Result<HWND> {
    unsafe {
        let hinstance =
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap();

        let class_name = windows::core::w!("FasterChantSearchPopup");

        // 窗口数据：需要传给 wndproc 的 Arc 引用
        let userdata = Box::new(SearchWindowData {
            input: input.clone(),
            results: results.clone(),
            selected: selected.clone(),
            on_select: on_select.clone(),
            schemes,
        });
        let userdata_ptr = Box::into_raw(userdata);

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(search_wndproc),
            hInstance: hinstance,
            hbrBackground: CreateSolidBrush(RGB(30, 30, 35)),
            lpszClassName: class_name,
            ..Default::default()
        };

        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            windows::core::w!("切换英雄"),
            WS_POPUP | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            None,
            None,
            hinstance,
            Some(userdata_ptr as *const _ as _),
        );

        if hwnd.0 == 0 {
            anyhow::bail!("创建搜索窗口失败");
        }

        Ok(hwnd)
    }
}

struct SearchWindowData {
    input: Arc<Mutex<String>>,
    results: Arc<Mutex<Vec<String>>>,
    selected: Arc<Mutex<usize>>,
    on_select: Arc<Mutex<Option<SearchCallback>>>,
    schemes: Arc<SchemeManager>,
}

unsafe extern "system" fn search_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            // 保存窗口数据指针
            let cs = &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
            windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::GWL_USERDATA,
                cs.lpCreateParams as isize,
            );
            LRESULT(0)
        }
        WM_NCHITTEST => {
            // 整个窗口可拖动
            LRESULT(HTCAPTION as isize)
        }
        WM_ERASEBKGND => LRESULT(1), // 自定义绘制
        WM_PAINT => {
            let data = get_data(hwnd);
            paint(hwnd, &data);
            LRESULT(0)
        }
        WM_CHAR => {
            let data = get_data(hwnd);
            let c = char::from_u32(wparam.0 as u32).unwrap_or('\0');

            if c == '\u{8}' {
                // 退格
                data.input.lock().unwrap().pop();
            } else if c == '\r' {
                // 回车：确认选择
                confirm_selection(&data);
                return LRESULT(0);
            } else if c == '\u{1b}' {
                // Esc：关闭
                hide_window(hwnd, &data);
                return LRESULT(0);
            } else if c.is_ascii_graphic() || c == ' ' {
                data.input.lock().unwrap().push(c);
            }

            // 更新搜索结果
            let input = data.input.lock().unwrap().clone();
            let new_results = data.schemes.search_pinyin(&input);
            *data.results.lock().unwrap() = new_results;
            *data.selected.lock().unwrap() = 0;

            InvalidateRect(hwnd, None, true);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let data = get_data(hwnd);
            let vk = wparam.0 as u32;
            let mut results = data.results.lock().unwrap();
            let mut sel = data.selected.lock().unwrap();

            match vk {
                VK_UP => {
                    if *sel > 0 {
                        *sel -= 1;
                    }
                    InvalidateRect(hwnd, None, true);
                }
                VK_DOWN => {
                    if *sel + 1 < results.len() {
                        *sel += 1;
                    }
                    InvalidateRect(hwnd, None, true);
                }
                VK_RETURN => {
                    confirm_selection(&data);
                }
                VK_ESCAPE => {
                    hide_window(hwnd, &data);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_KILLFOCUS => {
            let data = get_data(hwnd);
            hide_window(hwnd, &data);
            LRESULT(0)
        }
        WM_DESTROY => {
            let data = get_data(hwnd);
            // 释放 Box
            let _ = Box::from_raw(
                windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
                    hwnd,
                    windows::Win32::UI::WindowsAndMessaging::GWL_USERDATA,
                ) as *mut SearchWindowData,
            );
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn get_data(hwnd: HWND) -> &'static SearchWindowData {
    let ptr = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
        hwnd,
        windows::Win32::UI::WindowsAndMessaging::GWL_USERDATA,
    ) as *mut SearchWindowData;
    &*ptr
}

fn confirm_selection(data: &SearchWindowData) {
    let results = data.results.lock().unwrap();
    let sel = *data.selected.lock().unwrap();
    if sel < results.len() {
        let name = results[sel].clone();
        info!("校准选择: {}", name);
        if let Some(ref cb) = *data.on_select.lock().unwrap() {
            cb(&name);
        }
    }
}

fn hide_window(hwnd: HWND, _data: &SearchWindowData) {
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
}

unsafe fn paint(hwnd: HWND, data: &SearchWindowData) {
    let mut rect = RECT::default();
    GetClientRect(hwnd, &mut rect);

    let hdc = windows::Win32::Graphics::Gdi::GetDC(hwnd);
    let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
    let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);

    // 背景
    let bg_brush = CreateSolidBrush(RGB(30, 30, 35));
    FillRect(hdc, &rect, bg_brush);
    DeleteObject(bg_brush);

    // 选择字体
    let font = GetStockObject(DEFAULT_GUI_FONT);
    let old_font = SelectObject(hdc, font);
    SetBkMode(hdc, TRANSPARENT);

    // 输入框背景
    let input_rect = RECT {
        left: PADDING,
        top: PADDING,
        right: rect.right - PADDING,
        bottom: PADDING + INPUT_HEIGHT,
    };
    let input_bg = CreateSolidBrush(RGB(50, 50, 58));
    FillRect(hdc, &input_rect, input_bg);
    DeleteObject(input_bg);

    // 输入框边框
    let border = CreateSolidBrush(RGB(80, 80, 180));
    let border_rect = RECT {
        left: PADDING - 1,
        top: PADDING - 1,
        right: rect.right - PADDING + 1,
        bottom: PADDING + INPUT_HEIGHT + 1,
    };
    windows::Win32::Graphics::Gdi::FrameRect(hdc, &border_rect, border);
    DeleteObject(border);

    // 输入文字
    let input = data.input.lock().unwrap();
    SetTextColor(hdc, RGB(255, 255, 255));
    let text_rect = RECT {
        left: PADDING + 6,
        top: PADDING + 4,
        right: rect.right - PADDING - 6,
        bottom: PADDING + INPUT_HEIGHT,
    };
    let text: Vec<u16> = if input.is_empty() {
        "输入拼音或英雄名...".encode_utf16().collect()
    } else {
        format!("{}", *input).encode_utf16().collect()
    };
    if input.is_empty() {
        SetTextColor(hdc, RGB(100, 100, 110));
    }
    windows::Win32::Graphics::Gdi::DrawTextW(
        hdc,
        &text,
        &mut text_rect.clone(),
        windows::Win32::Graphics::Gdi::DT_LEFT
            | windows::Win32::Graphics::Gdi::DT_VCENTER
            | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
    );
    drop(input);

    // 结果列表
    let results = data.results.lock().unwrap();
    let sel = *data.selected.lock().unwrap();
    let list_top = PADDING + INPUT_HEIGHT + 12;

    for (i, name) in results.iter().enumerate() {
        let item_rect = RECT {
            left: PADDING,
            top: list_top + i as i32 * ITEM_HEIGHT,
            right: rect.right - PADDING,
            bottom: list_top + (i as i32 + 1) * ITEM_HEIGHT,
        };

        if i == sel {
            // 选中高亮
            let sel_bg = CreateSolidBrush(RGB(80, 80, 180));
            FillRect(hdc, &item_rect, sel_bg);
            DeleteObject(sel_bg);
            SetTextColor(hdc, RGB(255, 255, 255));
        } else {
            SetTextColor(hdc, RGB(200, 200, 210));
        }

        let name_wide: Vec<u16> = name.encode_utf16().collect();
        let mut text_rect = RECT {
            left: PADDING + 8,
            top: item_rect.top + 4,
            right: item_rect.right - 8,
            bottom: item_rect.bottom - 4,
        };
        windows::Win32::Graphics::Gdi::DrawTextW(
            hdc,
            &name_wide,
            &mut text_rect,
            windows::Win32::Graphics::Gdi::DT_LEFT
                | windows::Win32::Graphics::Gdi::DT_VCENTER
                | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
        );
    }

    SelectObject(hdc, old_font);
    windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
}