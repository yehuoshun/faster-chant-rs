use crate::scheme::scheme::SchemeManager;
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
    WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE, HTCAPTION, VK_DOWN, VK_ESCAPE, VK_LEFT,
    VK_RETURN, VK_RIGHT, VK_UP,
};

const WINDOW_WIDTH: i32 = 340;
const WINDOW_HEIGHT: i32 = 380;
const PADDING: i32 = 12;
const INPUT_HEIGHT: i32 = 32;
const HEADER_HEIGHT: i32 = 22;
const ROW_HEIGHT: i32 = 26;
const COL_HERO: i32 = 110;
const COL_SEP: i32 = 16;
const SEP_WIDTH: i32 = 1;

/// 英雄 + 皮肤
#[derive(Debug, Clone)]
struct HeroItem {
    name: String,
    skins: Vec<SkinItem>,
}

#[derive(Debug, Clone)]
struct SkinItem {
    display_name: String,
    skin_name: String,
}

pub type SearchCallback = Box<dyn Fn(&str) + Send + Sync>;

pub struct SearchPopup {
    hwnd: HWND,
    input: Arc<Mutex<String>>,
    heroes: Arc<Mutex<Vec<HeroItem>>>,
    hero_sel: Arc<Mutex<usize>>,
    skin_sel: Arc<Mutex<usize>>,
    /// 光标在左边（英雄）还是右边（皮肤）
    focus: Arc<Mutex<PanelFocus>>,
    schemes: Arc<SchemeManager>,
    on_select: Arc<Mutex<Option<SearchCallback>>>,
}

#[derive(Debug, Clone, PartialEq)]
enum PanelFocus {
    Hero,
    Skin,
}

impl SearchPopup {
    pub fn new(schemes: Arc<SchemeManager>) -> Result<Self> {
        let input = Arc::new(Mutex::new(String::new()));
        let heroes = Arc::new(Mutex::new(Vec::new()));
        let hero_sel = Arc::new(Mutex::new(0usize));
        let skin_sel = Arc::new(Mutex::new(0usize));
        let focus = Arc::new(Mutex::new(PanelFocus::Hero));
        let on_select: Arc<Mutex<Option<SearchCallback>>> = Arc::new(Mutex::new(None));

        let hwnd = create_window(
            &input, &heroes, &hero_sel, &skin_sel, &focus, &on_select, schemes.clone(),
        )?;

        Ok(Self {
            hwnd, input, heroes, hero_sel, skin_sel, focus, schemes, on_select,
        })
    }

    pub fn show(&self) {
        *self.input.lock().unwrap() = String::new();
        *self.hero_sel.lock().unwrap() = 0;
        *self.skin_sel.lock().unwrap() = 0;
        *self.focus.lock().unwrap() = PanelFocus::Hero;
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
                self.hwnd, HWND_TOPMOST, x, y, WINDOW_WIDTH, WINDOW_HEIGHT,
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
        *self.heroes.lock().unwrap() = group_heroes(&schemes, input);
        *self.hero_sel.lock().unwrap() = 0;
        *self.skin_sel.lock().unwrap() = 0;
    }
}

impl Drop for SearchPopup {
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.hwnd).ok(); }
    }
}

fn group_heroes(
    schemes: &[&crate::scheme::scheme::HeroScheme],
    input: &str,
) -> Vec<HeroItem> {
    let input = input.to_lowercase();
    let mut hero_map: HashMap<String, Vec<SkinItem>> = HashMap::new();

    for scheme in schemes {
        if !input.is_empty() {
            let pinyin = crate::scheme::scheme::to_pinyin_initials(&scheme.hero_name).to_lowercase();
            let matched = scheme.hero_name.contains(&input)
                || pinyin.contains(&input)
                || scheme.skin_name.as_deref().map(|s| s.contains(&input)).unwrap_or(false);
            if !matched {
                continue;
            }
        }
        let entry = hero_map.entry(scheme.hero_name.clone()).or_default();
        entry.push(SkinItem {
            display_name: scheme.display_name.clone(),
            skin_name: scheme.skin_name.clone().unwrap_or_else(|| "原皮".to_string()),
        });
    }

    let mut heroes: Vec<HeroItem> = hero_map
        .into_iter()
        .map(|(name, mut skins)| {
            skins.sort_by(|a, b| {
                let a_def = a.skin_name == "原皮";
                let b_def = b.skin_name == "原皮";
                b_def.cmp(&a_def).then_with(|| a.skin_name.cmp(&b.skin_name))
            });
            HeroItem { name, skins }
        })
        .collect();

    heroes.sort_by(|a, b| a.name.cmp(&b.name));
    heroes
}

// ── 窗口 ──

fn create_window(
    input: &Arc<Mutex<String>>,
    heroes: &Arc<Mutex<Vec<HeroItem>>>,
    hero_sel: &Arc<Mutex<usize>>,
    skin_sel: &Arc<Mutex<usize>>,
    focus: &Arc<Mutex<PanelFocus>>,
    on_select: &Arc<Mutex<Option<SearchCallback>>>,
    schemes: Arc<SchemeManager>,
) -> Result<HWND> {
    unsafe {
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap();
        let class_name = windows::core::w!("FasterChantSearchPopup");

        let userdata = Box::new(SearchWindowData {
            input: input.clone(),
            heroes: heroes.clone(),
            hero_sel: hero_sel.clone(),
            skin_sel: skin_sel.clone(),
            focus: focus.clone(),
            on_select: on_select.clone(),
            schemes,
        });
        let ptr = Box::into_raw(userdata);

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
            CW_USEDEFAULT, CW_USEDEFAULT, WINDOW_WIDTH, WINDOW_HEIGHT,
            None, None, hinstance,
            Some(ptr as *const _ as _),
        );

        if hwnd.0 == 0 {
            anyhow::bail!("创建搜索窗口失败");
        }
        Ok(hwnd)
    }
}

struct SearchWindowData {
    input: Arc<Mutex<String>>,
    heroes: Arc<Mutex<Vec<HeroItem>>>,
    hero_sel: Arc<Mutex<usize>>,
    skin_sel: Arc<Mutex<usize>>,
    focus: Arc<Mutex<PanelFocus>>,
    on_select: Arc<Mutex<Option<SearchCallback>>>,
    schemes: Arc<SchemeManager>,
}

// ── 窗口过程 ──

unsafe extern "system" fn search_wndproc(
    hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let cs = &*(lparam.0 as *const windows::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
            windows::Win32::UI::WindowsAndMessaging::SetWindowLongPtrW(
                hwnd, windows::Win32::UI::WindowsAndMessaging::GWL_USERDATA, cs.lpCreateParams as isize,
            );
            LRESULT(0)
        }
        WM_NCHITTEST => LRESULT(HTCAPTION as isize),
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => { let d = get_data(hwnd); paint(hwnd, &d); LRESULT(0) }
        WM_CHAR => {
            let d = get_data(hwnd);
            let c = char::from_u32(wparam.0 as u32).unwrap_or('\0');
            if c == '\u{1b}' {
                hide_window(hwnd);
                return LRESULT(0);
            }
            if c == '\r' {
                handle_enter(&d);
                return LRESULT(0);
            }
            if c == '\u{8}' {
                d.input.lock().unwrap().pop();
            } else if c.is_ascii_graphic() || c == ' ' {
                d.input.lock().unwrap().push(c);
            } else {
                return LRESULT(0);
            }
            update_search(&d);
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let d = get_data(hwnd);
            let vk = wparam.0 as u32;
            let focus = d.focus.lock().unwrap().clone();
            let heroes = d.heroes.lock().unwrap();

            match (focus, vk) {
                // 英雄列
                (PanelFocus::Hero, VK_UP) => {
                    let mut sel = d.hero_sel.lock().unwrap();
                    if *sel > 0 { *sel -= 1; *d.skin_sel.lock().unwrap() = 0; }
                    InvalidateRect(hwnd, None, true);
                }
                (PanelFocus::Hero, VK_DOWN) => {
                    let mut sel = d.hero_sel.lock().unwrap();
                    if *sel + 1 < heroes.len() { *sel += 1; *d.skin_sel.lock().unwrap() = 0; }
                    InvalidateRect(hwnd, None, true);
                }
                (PanelFocus::Hero, VK_RIGHT) => {
                    *d.focus.lock().unwrap() = PanelFocus::Skin;
                    InvalidateRect(hwnd, None, true);
                }
                (PanelFocus::Hero, VK_RETURN) => {
                    let sel = *d.hero_sel.lock().unwrap();
                    if let Some(hero) = heroes.get(sel) {
                        if hero.skins.len() == 1 {
                            // 单皮肤，直接选中
                            confirm(&hero.skins[0].display_name, &d);
                        } else {
                            // 多皮肤，跳到右边
                            *d.focus.lock().unwrap() = PanelFocus::Skin;
                            *d.skin_sel.lock().unwrap() = 0;
                            InvalidateRect(hwnd, None, true);
                        }
                    }
                }
                (PanelFocus::Hero, VK_ESCAPE) => { hide_window(hwnd); }

                // 皮肤列
                (PanelFocus::Skin, VK_UP) => {
                    let mut sel = d.skin_sel.lock().unwrap();
                    if *sel > 0 { *sel -= 1; }
                    InvalidateRect(hwnd, None, true);
                }
                (PanelFocus::Skin, VK_DOWN) => {
                    let hero_sel = *d.hero_sel.lock().unwrap();
                    let count = heroes.get(hero_sel).map(|h| h.skins.len()).unwrap_or(0);
                    let mut skin_sel = d.skin_sel.lock().unwrap();
                    if *skin_sel + 1 < count { *skin_sel += 1; }
                    InvalidateRect(hwnd, None, true);
                }
                (PanelFocus::Skin, VK_LEFT) => {
                    *d.focus.lock().unwrap() = PanelFocus::Hero;
                    InvalidateRect(hwnd, None, true);
                }
                (PanelFocus::Skin, VK_RETURN) => {
                    let hero_sel = *d.hero_sel.lock().unwrap();
                    let skin_sel = *d.skin_sel.lock().unwrap();
                    if let Some(hero) = heroes.get(hero_sel) {
                        if let Some(skin) = hero.skins.get(skin_sel) {
                            confirm(&skin.display_name, &d);
                        }
                    }
                }
                (PanelFocus::Skin, VK_ESCAPE) => {
                    *d.focus.lock().unwrap() = PanelFocus::Hero;
                    InvalidateRect(hwnd, None, true);
                }
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
    *data.heroes.lock().unwrap() = group_heroes(&schemes, &input);
    *data.hero_sel.lock().unwrap() = 0;
    *data.skin_sel.lock().unwrap() = 0;
    *data.focus.lock().unwrap() = PanelFocus::Hero;
    unsafe {
        InvalidateRect(
            windows::Win32::UI::WindowsAndMessaging::GetWindow(
                windows::Win32::UI::WindowsAndMessaging::GW_HWNDFIRST,
                windows::Win32::UI::WindowsAndMessaging::GW_HWNDFIRST,
            ),
            None, true,
        );
    }
}

fn handle_enter(data: &SearchWindowData) {
    let focus = data.focus.lock().unwrap().clone();
    let heroes = data.heroes.lock().unwrap();
    match focus {
        PanelFocus::Hero => {
            let sel = *data.hero_sel.lock().unwrap();
            if let Some(hero) = heroes.get(sel) {
                if hero.skins.len() == 1 {
                    confirm(&hero.skins[0].display_name, data);
                } else {
                    *data.focus.lock().unwrap() = PanelFocus::Skin;
                    *data.skin_sel.lock().unwrap() = 0;
                    unsafe {
                        InvalidateRect(
                            windows::Win32::UI::WindowsAndMessaging::GetWindow(
                                windows::Win32::UI::WindowsAndMessaging::GW_HWNDFIRST,
                                windows::Win32::UI::WindowsAndMessaging::GW_HWNDFIRST,
                            ),
                            None, true,
                        );
                    }
                }
            }
        }
        PanelFocus::Skin => {
            let hero_sel = *data.hero_sel.lock().unwrap();
            let skin_sel = *data.skin_sel.lock().unwrap();
            if let Some(hero) = heroes.get(hero_sel) {
                if let Some(skin) = hero.skins.get(skin_sel) {
                    confirm(&skin.display_name, data);
                }
            }
        }
    }
}

fn confirm(name: &str, data: &SearchWindowData) {
    info!("校准选择: {}", name);
    if let Some(ref cb) = *data.on_select.lock().unwrap() {
        cb(name);
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
        left: PADDING, top: PADDING,
        right: rect.right - PADDING, bottom: PADDING + INPUT_HEIGHT,
    };
    let input_bg = CreateSolidBrush(RGB(45, 45, 52));
    FillRect(hdc, &input_rect, input_bg);
    DeleteObject(input_bg);

    let border = CreateSolidBrush(RGB(80, 80, 180));
    let br = RECT {
        left: PADDING - 1, top: PADDING - 1,
        right: rect.right - PADDING + 1, bottom: PADDING + INPUT_HEIGHT + 1,
    };
    windows::Win32::Graphics::Gdi::FrameRect(hdc, &br, border);
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
        windows::Win32::Graphics::Gdi::DT_LEFT | windows::Win32::Graphics::Gdi::DT_VCENTER
            | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
    );
    drop(input);

    // ── 表头 ──
    let header_y = PADDING + INPUT_HEIGHT + 8;
    SetTextColor(hdc, RGB(120, 120, 140));
    let hh: Vec<u16> = "英雄".encode_utf16().collect();
    let mut hr = RECT {
        left: PADDING + 8, top: header_y,
        right: PADDING + COL_HERO, bottom: header_y + HEADER_HEIGHT,
    };
    windows::Win32::Graphics::Gdi::DrawTextW(
        hdc, &hh, &mut hr,
        windows::Win32::Graphics::Gdi::DT_LEFT | windows::Win32::Graphics::Gdi::DT_VCENTER
            | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
    );
    let hs: Vec<u16> = "皮肤".encode_utf16().collect();
    let mut sr = RECT {
        left: PADDING + COL_HERO + COL_SEP, top: header_y,
        right: rect.right - PADDING, bottom: header_y + HEADER_HEIGHT,
    };
    windows::Win32::Graphics::Gdi::DrawTextW(
        hdc, &hs, &mut sr,
        windows::Win32::Graphics::Gdi::DT_LEFT | windows::Win32::Graphics::Gdi::DT_VCENTER
            | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
    );

    // 分隔线
    let sep_y = header_y + HEADER_HEIGHT;
    let sep = CreateSolidBrush(RGB(55, 55, 60));
    let sr2 = RECT {
        left: PADDING, top: sep_y,
        right: rect.right - PADDING, bottom: sep_y + 1,
    };
    FillRect(hdc, &sr2, sep);
    DeleteObject(sep);

    let list_top = sep_y + 2;
    let heroes = data.heroes.lock().unwrap();
    let hero_sel = *data.hero_sel.lock().unwrap();
    let skin_sel = *data.skin_sel.lock().unwrap();
    let focus = data.focus.lock().unwrap().clone();

    // ── 英雄列 ──
    let mut y = list_top;
    for (i, hero) in heroes.iter().enumerate() {
        // 计算该英雄占几行（选中英雄的多皮肤需要展开）
        let row_count = if i == hero_sel { hero.skins.len().max(1) } else { 1 };

        for r in 0..row_count {
            let row_rect = RECT {
                left: PADDING, top: y,
                right: PADDING + COL_HERO, bottom: y + ROW_HEIGHT,
            };

            let is_active = focus == PanelFocus::Hero && i == hero_sel && r == 0;
            if is_active {
                let sel_bg = CreateSolidBrush(RGB(70, 70, 160));
                FillRect(hdc, &row_rect, sel_bg);
                DeleteObject(sel_bg);
                SetTextColor(hdc, RGB(255, 255, 255));
            } else {
                if (i + r) % 2 == 0 {
                    let alt = CreateSolidBrush(RGB(34, 34, 39));
                    FillRect(hdc, &row_rect, alt);
                    DeleteObject(alt);
                }
                SetTextColor(hdc, RGB(200, 200, 215));
            }

            // 只在第一行显示英雄名，其余行留空
            if r == 0 {
                let name: Vec<u16> = hero.name.encode_utf16().collect();
                let mut nr = RECT {
                    left: PADDING + 8, top: y + 2,
                    right: PADDING + COL_HERO - 8, bottom: y + ROW_HEIGHT - 2,
                };
                windows::Win32::Graphics::Gdi::DrawTextW(
                    hdc, &name, &mut nr,
                    windows::Win32::Graphics::Gdi::DT_LEFT | windows::Win32::Graphics::Gdi::DT_VCENTER
                        | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
                );
            }

            y += ROW_HEIGHT;
        }
    }

    // ── 垂直分隔线 ──
    let vline = CreateSolidBrush(RGB(55, 55, 60));
    let vr = RECT {
        left: PADDING + COL_HERO + COL_SEP / 2,
        top: list_top,
        right: PADDING + COL_HERO + COL_SEP / 2 + SEP_WIDTH,
        bottom: rect.bottom - PADDING,
    };
    FillRect(hdc, &vr, vline);
    DeleteObject(vline);

    // ── 皮肤列：从选中英雄的行开始 ──
    if let Some(hero) = heroes.get(hero_sel) {
        // 计算选中英雄的 y 偏移（累计前面英雄的行数）
        let mut skin_y = list_top;
        for i in 0..hero_sel {
            let row_count = if i == hero_sel { heroes[i].skins.len().max(1) } else { 1 };
            skin_y += row_count as i32 * ROW_HEIGHT;
        }

        for (i, skin) in hero.skins.iter().enumerate() {
            let row_rect = RECT {
                left: PADDING + COL_HERO + COL_SEP,
                top: skin_y,
                right: rect.right - PADDING,
                bottom: skin_y + ROW_HEIGHT,
            };

            let is_active = focus == PanelFocus::Skin && i == skin_sel;
            if is_active {
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
                SetTextColor(hdc, RGB(170, 170, 185));
            }

            let skin_text: Vec<u16> = skin.skin_name.encode_utf16().collect();
            let mut sr = RECT {
                left: row_rect.left + 8,
                top: skin_y + 2,
                right: row_rect.right - 8,
                bottom: skin_y + ROW_HEIGHT - 2,
            };
            windows::Win32::Graphics::Gdi::DrawTextW(
                hdc, &skin_text, &mut sr,
                windows::Win32::Graphics::Gdi::DT_LEFT | windows::Win32::Graphics::Gdi::DT_VCENTER
                    | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
            );

            skin_y += ROW_HEIGHT;
        }
    }

    SelectObject(hdc, old_font);
    windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
}