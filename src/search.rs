use crate::scheme::SchemeManager;
use anyhow::Result;
use log::info;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, DeleteObject, GetStockObject, SelectObject, SetBkMode, SetTextColor,
    DEFAULT_GUI_FONT, RGB, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, InvalidateRect,
    RegisterClassW, SetWindowPos, ShowWindow, WNDCLASSW, CS_HREDRAW, CS_VREDRAW,
    CW_USEDEFAULT, HWND_TOPMOST, SW_HIDE, SW_SHOW, WM_CHAR, WM_CREATE, WM_DESTROY,
    WM_ERASEBKGND, WM_KEYDOWN, WM_KILLFOCUS, WM_NCHITTEST, WM_PAINT, WS_EX_TOPMOST,
    WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE, HTCAPTION, VK_DOWN, VK_ESCAPE, VK_RETURN, VK_UP,
};

const WINDOW_WIDTH: i32 = 360;
const WINDOW_HEIGHT: i32 = 420;
const PADDING: i32 = 12;
const INPUT_HEIGHT: i32 = 32;
const HEADER_HEIGHT: i32 = 22;
const ROW_HEIGHT: i32 = 24;
const COL_HERO: i32 = 100;
const COL_SEP: i32 = 20;

/// 搜索结果行
#[derive(Debug, Clone)]
pub struct ResultRow {
    pub hero_name: String,
    pub skin_name: String,
    pub display_name: String,
    /// 该英雄共有几个皮肤
    pub skin_count: usize,
}

pub type SearchCallback = Box<dyn Fn(&str) + Send + Sync>;

pub struct SearchPopup {
    hwnd: HWND,
    input: Arc<Mutex<String>>,
    rows: Arc<Mutex<Vec<ResultRow>>>,
    selected: Arc<Mutex<usize>>,
    schemes: Arc<SchemeManager>,
    on_select: Arc<Mutex<Option<SearchCallback>>>,
}

impl SearchPopup {
    pub fn new(schemes: Arc<SchemeManager>) -> Result<Self> {
        let input = Arc::new(Mutex::new(String::new()));
        let rows = Arc::new(Mutex::new(Vec::new()));
        let selected = Arc::new(Mutex::new(0usize));
        let on_select: Arc<Mutex<Option<SearchCallback>>> = Arc::new(Mutex::new(None));

        let hwnd = create_window(&input, &rows, &selected, &on_select, schemes.clone())?;

        Ok(Self {
            hwnd,
            input,
            rows,
            selected,
            schemes,
            on_select,
        })
    }

    pub fn show(&self) {
        *self.input.lock().unwrap() = String::new();
        self.refresh("");

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
        unsafe { ShowWindow(self.hwnd, SW_HIDE); }
    }

    pub fn set_callback<F: Fn(&str) + Send + Sync + 'static>(&self, f: F) {
        *self.on_select.lock().unwrap() = Some(Box::new(f));
    }

    fn refresh(&self, input: &str) {
        let schemes = self.schemes.all();
        let rows = build_rows(&schemes, input);
        *self.rows.lock().unwrap() = rows;
        *self.selected.lock().unwrap() = 0;
    }
}

impl Drop for SearchPopup {
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.hwnd).ok(); }
    }
}

/// 搜索匹配 → 扁平行列表
fn build_rows(schemes: &[&crate::scheme::HeroScheme], input: &str) -> Vec<ResultRow> {
    let input = input.to_lowercase();
    let mut rows: Vec<ResultRow> = Vec::new();

    for scheme in schemes {
        if !input.is_empty() {
            let pinyin = crate::scheme::to_pinyin_initials(&scheme.hero_name).to_lowercase();
            let skin_pinyin = scheme
                .skin_name
                .as_deref()
                .map(|s| crate::scheme::to_pinyin_initials(s).to_lowercase());
            let matched = scheme.hero_name.contains(&input)
                || pinyin.contains(&input)
                || scheme.display_name.contains(&input)
                || skin_pinyin
                    .as_ref()
                    .map(|sp| sp.contains(&input))
                    .unwrap_or(false)
                || scheme
                    .skin_name
                    .as_deref()
                    .map(|s| s.contains(&input))
                    .unwrap_or(false);
            if !matched {
                continue;
            }
        }

        rows.push(ResultRow {
            hero_name: scheme.hero_name.clone(),
            skin_name: scheme.skin_name.clone().unwrap_or_else(|| "原皮".to_string()),
            display_name: scheme.display_name.clone(),
        });
    }

    // 排序：英雄名 > 皮肤名（原皮优先）
    rows.sort_by(|a, b| {
        a.hero_name
            .cmp(&b.hero_name)
            .then_with(|| {
                let a_is_default = a.skin_name == "原皮";
                let b_is_default = b.skin_name == "原皮";
                b_is_default
                    .cmp(&a_is_default)
                    .then_with(|| a.skin_name.cmp(&b.skin_name))
            })
    });

    // 计算每个英雄的皮肤数量
    let mut hero_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for row in &rows {
        *hero_counts.entry(row.hero_name.clone()).or_default() += 1;
    }
    for row in &mut rows {
        row.skin_count = hero_counts.get(&row.hero_name).copied().unwrap_or(1);
    }

    rows
}

// ── 窗口创建 ──

fn create_window(
    input: &Arc<Mutex<String>>,
    rows: &Arc<Mutex<Vec<ResultRow>>>,
    selected: &Arc<Mutex<usize>>,
    on_select: &Arc<Mutex<Option<SearchCallback>>>,
    schemes: Arc<SchemeManager>,
) -> Result<HWND> {
    unsafe {
        let hinstance =
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap();
        let class_name = windows::core::w!("FasterChantSearchPopup");

        let userdata = Box::new(SearchWindowData {
            input: input.clone(),
            rows: rows.clone(),
            selected: selected.clone(),
            on_select: on_select.clone(),
            schemes,
        });
        let userdata_ptr = Box::into_raw(userdata);

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(search_wndproc),
            hInstance: hinstance,
            hbrBackground: CreateSolidBrush(RGB(28, 28, 33)),
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
    rows: Arc<Mutex<Vec<ResultRow>>>,
    selected: Arc<Mutex<usize>>,
    on_select: Arc<Mutex<Option<SearchCallback>>>,
    schemes: Arc<SchemeManager>,
}

// ── 窗口过程 ──

unsafe extern "system" fn search_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let cs = &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
            windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::GWL_USERDATA,
                cs.lpCreateParams as isize,
            );
            LRESULT(0)
        }
        WM_NCHITTEST => LRESULT(HTCAPTION as isize),
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => { let data = get_data(hwnd); paint(hwnd, &data); LRESULT(0) }
        WM_CHAR => {
            let data = get_data(hwnd);
            let c = char::from_u32(wparam.0 as u32).unwrap_or('\0');
            if c == '\u{8}' {
                data.input.lock().unwrap().pop();
            } else if c == '\u{1b}' {
                hide_window(hwnd);
                return LRESULT(0);
            } else if c == '\r' {
                confirm_selection(&data);
                return LRESULT(0);
            } else if c.is_ascii_graphic() || c == ' ' {
                data.input.lock().unwrap().push(c);
            } else {
                return LRESULT(0);
            }
            update_search(&data);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let data = get_data(hwnd);
            let vk = wparam.0 as u32;
            let len = data.rows.lock().unwrap().len();
            let mut sel = data.selected.lock().unwrap();
            match vk {
                VK_UP if *sel > 0 => { *sel -= 1; InvalidateRect(hwnd, None, true); }
                VK_DOWN if *sel + 1 < len => { *sel += 1; InvalidateRect(hwnd, None, true); }
                VK_RETURN => { confirm_selection(&data); }
                VK_ESCAPE => { hide_window(hwnd); }
                _ => {}
            }
            LRESULT(0)
        }
        WM_KILLFOCUS => { hide_window(hwnd); LRESULT(0) }
        WM_DESTROY => {
            let ptr = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
                hwnd, windows::Win32::UI::WindowsAndMessaging::GWL_USERDATA,
            ) as *mut SearchWindowData;
            let _ = Box::from_raw(ptr);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn get_data(hwnd: HWND) -> &'static SearchWindowData {
    let ptr = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
        hwnd, windows::Win32::UI::WindowsAndMessaging::GWL_USERDATA,
    ) as *mut SearchWindowData;
    &*ptr
}

fn update_search(data: &SearchWindowData) {
    let input = data.input.lock().unwrap().clone();
    let schemes = data.schemes.all();
    *data.rows.lock().unwrap() = build_rows(&schemes, &input);
    *data.selected.lock().unwrap() = 0;
    unsafe {
        InvalidateRect(
            windows::Win32::UI::WindowsAndMessaging::GetWindow(
                windows::Win32::UI::WindowsAndMessaging::GW_HWNDFIRST,
                windows::Win32::UI::WindowsAndMessaging::GW_HWNDFIRST,
            ),
            None,
            true,
        );
    }
}

fn confirm_selection(data: &SearchWindowData) {
    let rows = data.rows.lock().unwrap();
    let sel = *data.selected.lock().unwrap();
    if sel < rows.len() {
        let name = rows[sel].display_name.clone();
        info!("校准选择: {}", name);
        if let Some(ref cb) = *data.on_select.lock().unwrap() {
            cb(&name);
        }
    }
}

fn hide_window(hwnd: HWND) {
    unsafe { ShowWindow(hwnd, SW_HIDE); }
}

// ── 绘制 ──

unsafe fn paint(hwnd: HWND, data: &SearchWindowData) {
    let mut rect = RECT::default();
    GetClientRect(hwnd, &mut rect);

    let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
    let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);

    let bg = CreateSolidBrush(RGB(28, 28, 33));
    FillRect(hdc, &rect, bg);
    DeleteObject(bg);

    let font = GetStockObject(DEFAULT_GUI_FONT);
    let old_font = SelectObject(hdc, font);
    SetBkMode(hdc, TRANSPARENT);

    // ── 输入框 ──
    let input_rect = RECT {
        left: PADDING,
        top: PADDING,
        right: rect.right - PADDING,
        bottom: PADDING + INPUT_HEIGHT,
    };
    let input_bg = CreateSolidBrush(RGB(45, 45, 52));
    FillRect(hdc, &input_rect, input_bg);
    DeleteObject(input_bg);

    let border = CreateSolidBrush(RGB(80, 80, 180));
    let border_rect = RECT {
        left: PADDING - 1, top: PADDING - 1,
        right: rect.right - PADDING + 1, bottom: PADDING + INPUT_HEIGHT + 1,
    };
    windows::Win32::Graphics::Gdi::FrameRect(hdc, &border_rect, border);
    DeleteObject(border);

    let input = data.input.lock().unwrap();
    let text: Vec<u16> = if input.is_empty() {
        "输入拼音首字母搜索...".encode_utf16().collect()
    } else {
        input.encode_utf16().collect()
    };
    if input.is_empty() {
        SetTextColor(hdc, RGB(90, 90, 100));
    } else {
        SetTextColor(hdc, RGB(230, 230, 240));
    }
    let mut tr = RECT {
        left: PADDING + 8, top: PADDING + 4,
        right: rect.right - PADDING - 8, bottom: PADDING + INPUT_HEIGHT,
    };
    windows::Win32::Graphics::Gdi::DrawTextW(
        hdc, &text, &mut tr,
        windows::Win32::Graphics::Gdi::DT_LEFT
            | windows::Win32::Graphics::Gdi::DT_VCENTER
            | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
    );
    drop(input);

    // ── 表头 ──
    let header_y = PADDING + INPUT_HEIGHT + 8;
    SetTextColor(hdc, RGB(120, 120, 140));
    let header_hero: Vec<u16> = "英雄".encode_utf16().collect();
    let mut hr = RECT {
        left: PADDING + 8, top: header_y,
        right: PADDING + COL_HERO, bottom: header_y + HEADER_HEIGHT,
    };
    windows::Win32::Graphics::Gdi::DrawTextW(
        hdc, &header_hero, &mut hr,
        windows::Win32::Graphics::Gdi::DT_LEFT | windows::Win32::Graphics::Gdi::DT_VCENTER | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
    );

    let header_skin: Vec<u16> = "皮肤".encode_utf16().collect();
    let mut sr = RECT {
        left: PADDING + COL_HERO + COL_SEP, top: header_y,
        right: rect.right - PADDING, bottom: header_y + HEADER_HEIGHT,
    };
    windows::Win32::Graphics::Gdi::DrawTextW(
        hdc, &header_skin, &mut sr,
        windows::Win32::Graphics::Gdi::DT_LEFT | windows::Win32::Graphics::Gdi::DT_VCENTER | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
    );

    // 分隔线
    let sep_y = header_y + HEADER_HEIGHT;
    let sep_brush = CreateSolidBrush(RGB(55, 55, 60));
    let sep_rect = RECT {
        left: PADDING, top: sep_y,
        right: rect.right - PADDING, bottom: sep_y + 1,
    };
    FillRect(hdc, &sep_rect, sep_brush);
    DeleteObject(sep_brush);

    // ── 数据行 ──
    let rows = data.rows.lock().unwrap();
    let sel = *data.selected.lock().unwrap();
    let mut y = sep_y + 2;
    let mut last_hero = String::new();

    for (i, row) in rows.iter().enumerate() {
        let is_first_of_group = row.hero_name != last_hero;
        last_hero = row.hero_name.clone();

        let row_rect = RECT {
            left: PADDING, top: y,
            right: rect.right - PADDING, bottom: y + ROW_HEIGHT,
        };

        if i == sel {
            let sel_bg = CreateSolidBrush(RGB(70, 70, 160));
            FillRect(hdc, &row_rect, sel_bg);
            DeleteObject(sel_bg);
            SetTextColor(hdc, RGB(255, 255, 255));
        } else {
            if i % 2 == 0 {
                let alt = CreateSolidBrush(RGB(34, 34, 39));
                FillRect(hdc, &row_rect, alt);
                DeleteObject(alt);
            }
            SetTextColor(hdc, RGB(200, 200, 215));
        }

        // 英雄名：只在每组第一行显示
        if is_first_of_group {
            let hero: Vec<u16> = row.hero_name.encode_utf16().collect();
            let mut hr = RECT {
                left: PADDING + 8, top: y + 2,
                right: PADDING + COL_HERO, bottom: y + ROW_HEIGHT - 2,
            };
            windows::Win32::Graphics::Gdi::DrawTextW(
                hdc, &hero, &mut hr,
                windows::Win32::Graphics::Gdi::DT_LEFT | windows::Win32::Graphics::Gdi::DT_VCENTER
                    | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
            );
        }

        // 皮肤名：只有多皮肤英雄才显示皮肤名，单皮肤且原皮则留空
        let skin_text = if row.skin_name == "原皮" && row.skin_count <= 1 {
            String::new()
        } else {
            row.skin_name.clone()
        };
        SetTextColor(hdc, if i == sel { RGB(220, 220, 255) } else { RGB(170, 170, 185) });
        let skin: Vec<u16> = skin_text.encode_utf16().collect();
        let mut sr = RECT {
            left: PADDING + COL_HERO + COL_SEP, top: y + 2,
            right: rect.right - PADDING - 8, bottom: y + ROW_HEIGHT - 2,
        };
        windows::Win32::Graphics::Gdi::DrawTextW(
            hdc, &skin, &mut sr,
            windows::Win32::Graphics::Gdi::DT_LEFT | windows::Win32::Graphics::Gdi::DT_VCENTER
                | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
        );

        y += ROW_HEIGHT;
    }

    SelectObject(hdc, old_font);
    windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
}