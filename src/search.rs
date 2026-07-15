use crate::scheme::{HeroScheme, SchemeManager};
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

const WINDOW_WIDTH: i32 = 340;
const WINDOW_HEIGHT: i32 = 420;
const PADDING: i32 = 12;
const INPUT_HEIGHT: i32 = 32;
const HEADER_HEIGHT: i32 = 26;
const ITEM_HEIGHT: i32 = 24;
const INDENT: i32 = 20;

/// 搜索结果分组
#[derive(Debug, Clone)]
pub struct SearchGroup {
    pub hero_name: String,
    pub skins: Vec<SkinEntry>,
}

#[derive(Debug, Clone)]
pub struct SkinEntry {
    pub display_name: String,
    pub skin_name: String,
    pub is_default: bool,
}

/// 扁平化条目（渲染用）
#[derive(Debug, Clone)]
enum FlatItem {
    Hero(String),
    Skin(SkinEntry),
}

pub type SearchCallback = Box<dyn Fn(&str) + Send + Sync>;

pub struct SearchPopup {
    hwnd: HWND,
    input: Arc<Mutex<String>>,
    flat_items: Arc<Mutex<Vec<FlatItem>>>,
    selected: Arc<Mutex<usize>>,
    schemes: Arc<SchemeManager>,
    on_select: Arc<Mutex<Option<SearchCallback>>>,
}

impl SearchPopup {
    pub fn new(schemes: Arc<SchemeManager>) -> Result<Self> {
        let input = Arc::new(Mutex::new(String::new()));
        let flat_items = Arc::new(Mutex::new(Vec::new()));
        let selected = Arc::new(Mutex::new(0usize));
        let on_select: Arc<Mutex<Option<SearchCallback>>> = Arc::new(Mutex::new(None));

        let hwnd = create_window(&input, &flat_items, &selected, &on_select, schemes.clone())?;

        Ok(Self {
            hwnd,
            input,
            flat_items,
            selected,
            schemes,
            on_select,
        })
    }

    pub fn show(&self) {
        *self.input.lock().unwrap() = String::new();
        self.refresh_results("");

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

    fn refresh_results(&self, input: &str) {
        let schemes = self.schemes.all();
        let groups = group_by_hero(&schemes, input);
        let flat = flatten(&groups);
        *self.flat_items.lock().unwrap() = flat;
        *self.selected.lock().unwrap() = 0;
    }
}

impl Drop for SearchPopup {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.hwnd).ok();
        }
    }
}

/// 搜索并分组：输入匹配 → 按英雄名分组
fn group_by_hero(schemes: &[&HeroScheme], input: &str) -> Vec<SearchGroup> {
    let input = input.to_lowercase();
    let mut hero_map: HashMap<String, Vec<SkinEntry>> = HashMap::new();

    for scheme in schemes {
        let matched = if input.is_empty() {
            true
        } else {
            let pinyin = crate::scheme::to_pinyin_initials(&scheme.hero_name).to_lowercase();
            scheme.hero_name.contains(&input)
                || pinyin.contains(&input)
                || scheme
                    .skin_name
                    .as_deref()
                    .map(|s| s.contains(&input))
                    .unwrap_or(false)
        };

        if !matched {
            continue;
        }

        let entry = hero_map.entry(scheme.hero_name.clone()).or_default();
        entry.push(SkinEntry {
            display_name: scheme.display_name.clone(),
            skin_name: scheme.skin_name.clone().unwrap_or_else(|| "原皮".to_string()),
            is_default: scheme.skin_name.is_none(),
        });
    }

    // 排序：先按英雄名，皮肤按 is_default 优先
    let mut groups: Vec<SearchGroup> = hero_map
        .into_iter()
        .map(|(hero_name, skins)| {
            let mut skins = skins;
            skins.sort_by(|a, b| {
                b.is_default
                    .cmp(&a.is_default)
                    .then_with(|| a.skin_name.cmp(&b.skin_name))
            });
            SearchGroup { hero_name, skins }
        })
        .collect();

    groups.sort_by(|a, b| a.hero_name.cmp(&b.hero_name));
    groups
}

/// 扁平化：Hero 标题 + Skin 条目
fn flatten(groups: &[SearchGroup]) -> Vec<FlatItem> {
    let mut items = Vec::new();
    for g in groups {
        items.push(FlatItem::Hero(g.hero_name.clone()));
        for skin in &g.skins {
            items.push(FlatItem::Skin(skin.clone()));
        }
    }
    items
}

// ── 窗口创建 ──

fn create_window(
    input: &Arc<Mutex<String>>,
    flat_items: &Arc<Mutex<Vec<FlatItem>>>,
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
            flat_items: flat_items.clone(),
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
    flat_items: Arc<Mutex<Vec<FlatItem>>>,
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
        WM_PAINT => {
            let data = get_data(hwnd);
            paint(hwnd, &data);
            LRESULT(0)
        }
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
            let items = data.flat_items.lock().unwrap();
            let len = items.len();
            let mut sel = data.selected.lock().unwrap();

            match vk {
                VK_UP if *sel > 0 => {
                    *sel -= 1;
                    // 跳过 Hero 标题行
                    if let FlatItem::Hero(_) = &items[*sel] {
                        if *sel > 0 {
                            *sel -= 1;
                        }
                    }
                    InvalidateRect(hwnd, None, true);
                }
                VK_DOWN if *sel + 1 < len => {
                    *sel += 1;
                    if let FlatItem::Hero(_) = &items[*sel] {
                        if *sel + 1 < len {
                            *sel += 1;
                        }
                    }
                    InvalidateRect(hwnd, None, true);
                }
                VK_RETURN => {
                    confirm_selection(&data);
                }
                VK_ESCAPE => {
                    hide_window(hwnd);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_KILLFOCUS => {
            hide_window(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            let ptr = windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
                hwnd,
                windows::Win32::UI::WindowsAndMessaging::GWL_USERDATA,
            ) as *mut SearchWindowData;
            let _ = Box::from_raw(ptr);
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

fn update_search(data: &SearchWindowData) {
    let input = data.input.lock().unwrap().clone();
    let schemes = data.schemes.all();
    let groups = group_by_hero(&schemes, &input);
    *data.flat_items.lock().unwrap() = flatten(&groups);
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
    let items = data.flat_items.lock().unwrap();
    let sel = *data.selected.lock().unwrap();
    if sel < items.len() {
        if let FlatItem::Skin(ref skin) = items[sel] {
            info!("校准选择: {}", skin.display_name);
            if let Some(ref cb) = *data.on_select.lock().unwrap() {
                cb(&skin.display_name);
            }
        }
    }
}

fn hide_window(hwnd: HWND) {
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
}

// ── 绘制 ──

unsafe fn paint(hwnd: HWND, data: &SearchWindowData) {
    let mut rect = RECT::default();
    GetClientRect(hwnd, &mut rect);

    let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
    let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);

    // 背景
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
        left: PADDING - 1,
        top: PADDING - 1,
        right: rect.right - PADDING + 1,
        bottom: PADDING + INPUT_HEIGHT + 1,
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
    let mut text_rect = RECT {
        left: PADDING + 8,
        top: PADDING + 4,
        right: rect.right - PADDING - 8,
        bottom: PADDING + INPUT_HEIGHT,
    };
    windows::Win32::Graphics::Gdi::DrawTextW(
        hdc,
        &text,
        &mut text_rect,
        windows::Win32::Graphics::Gdi::DT_LEFT
            | windows::Win32::Graphics::Gdi::DT_VCENTER
            | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
    );
    drop(input);

    // ── 结果列表 ──
    let items = data.flat_items.lock().unwrap();
    let sel = *data.selected.lock().unwrap();
    let mut y = PADDING + INPUT_HEIGHT + 10;

    for (i, item) in items.iter().enumerate() {
        let (text, indent, is_hero) = match item {
            FlatItem::Hero(name) => (name.clone(), 0, true),
            FlatItem::Skin(skin) => {
                let label = format!("└ {}", skin.skin_name);
                (label, INDENT, false)
            }
        };

        let item_rect = RECT {
            left: PADDING + indent,
            top: y,
            right: rect.right - PADDING,
            bottom: y + if is_hero { HEADER_HEIGHT } else { ITEM_HEIGHT },
        };

        if i == sel {
            let sel_bg = CreateSolidBrush(RGB(70, 70, 160));
            let full_rect = RECT {
                left: PADDING,
                right: rect.right - PADDING,
                ..item_rect
            };
            FillRect(hdc, &full_rect, sel_bg);
            DeleteObject(sel_bg);
            SetTextColor(hdc, RGB(255, 255, 255));
        } else if is_hero {
            SetTextColor(hdc, RGB(160, 160, 180));
        } else {
            SetTextColor(hdc, RGB(200, 200, 215));
        }

        let text_wide: Vec<u16> = text.encode_utf16().collect();
        let mut tr = RECT {
            left: item_rect.left + 8,
            top: item_rect.top + 2,
            right: item_rect.right - 8,
            bottom: item_rect.bottom - 2,
        };
        windows::Win32::Graphics::Gdi::DrawTextW(
            hdc,
            &text_wide,
            &mut tr,
            windows::Win32::Graphics::Gdi::DT_LEFT
                | windows::Win32::Graphics::Gdi::DT_VCENTER
                | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
        );

        y += if is_hero { HEADER_HEIGHT } else { ITEM_HEIGHT };
    }

    SelectObject(hdc, old_font);
    windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
}

/// 确保 to_pinyin_initials 可访问
pub(crate) use crate::scheme::to_pinyin_initials;