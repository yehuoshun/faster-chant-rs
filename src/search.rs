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
    WS_EX_TOOLWINDOW, WS_POPUP, WS_VISIBLE, HTCAPTION, VK_DOWN, VK_ESCAPE, VK_LEFT,
    VK_RETURN, VK_RIGHT, VK_UP,
};

const WINDOW_WIDTH: i32 = 300;
const WINDOW_HEIGHT: i32 = 380;
const PADDING: i32 = 12;
const INPUT_HEIGHT: i32 = 32;
const ROW_HEIGHT: i32 = 28;

/// 二级菜单状态
#[derive(Debug, Clone)]
enum MenuLevel {
    /// 英雄列表
    Heroes,
    /// 皮肤列表（展开的皮肤名）
    Skins,
}

/// 英雄列表项
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
    selected_hero: Arc<Mutex<usize>>,
    selected_skin: Arc<Mutex<usize>>,
    level: Arc<Mutex<MenuLevel>>,
    schemes: Arc<SchemeManager>,
    on_select: Arc<Mutex<Option<SearchCallback>>>,
}

impl SearchPopup {
    pub fn new(schemes: Arc<SchemeManager>) -> Result<Self> {
        let input = Arc::new(Mutex::new(String::new()));
        let heroes = Arc::new(Mutex::new(Vec::new()));
        let selected_hero = Arc::new(Mutex::new(0usize));
        let selected_skin = Arc::new(Mutex::new(0usize));
        let level = Arc::new(Mutex::new(MenuLevel::Heroes));
        let on_select: Arc<Mutex<Option<SearchCallback>>> = Arc::new(Mutex::new(None));

        let hwnd = create_window(
            &input, &heroes, &selected_hero, &selected_skin, &level, &on_select, schemes.clone(),
        )?;

        Ok(Self {
            hwnd,
            input,
            heroes,
            selected_hero,
            selected_skin,
            level,
            schemes,
            on_select,
        })
    }

    pub fn show(&self) {
        *self.input.lock().unwrap() = String::new();
        *self.level.lock().unwrap() = MenuLevel::Heroes;
        *self.selected_hero.lock().unwrap() = 0;
        *self.selected_skin.lock().unwrap() = 0;
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
        *self.heroes.lock().unwrap() = build_heroes(&schemes, input);
        *self.selected_hero.lock().unwrap() = 0;
        *self.selected_skin.lock().unwrap() = 0;
    }
}

impl Drop for SearchPopup {
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.hwnd).ok(); }
    }
}

fn build_heroes(
    schemes: &[&crate::scheme::HeroScheme],
    input: &str,
) -> Vec<HeroItem> {
    let input = input.to_lowercase();
    let mut hero_map: HashMap<String, Vec<SkinItem>> = HashMap::new();

    for scheme in schemes {
        if !input.is_empty() {
            let pinyin = crate::scheme::to_pinyin_initials(&scheme.hero_name).to_lowercase();
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
                let a_is_default = a.skin_name == "原皮";
                let b_is_default = b.skin_name == "原皮";
                b_is_default.cmp(&a_is_default).then_with(|| a.skin_name.cmp(&b.skin_name))
            });
            HeroItem { name, skins }
        })
        .collect();

    heroes.sort_by(|a, b| a.name.cmp(&b.name));
    heroes
}

// ── 窗口创建 ──

fn create_window(
    input: &Arc<Mutex<String>>,
    heroes: &Arc<Mutex<Vec<HeroItem>>>,
    selected_hero: &Arc<Mutex<usize>>,
    selected_skin: &Arc<Mutex<usize>>,
    level: &Arc<Mutex<MenuLevel>>,
    on_select: &Arc<Mutex<Option<SearchCallback>>>,
    schemes: Arc<SchemeManager>,
) -> Result<HWND> {
    unsafe {
        let hinstance =
            windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap();
        let class_name = windows::core::w!("FasterChantSearchPopup");

        let userdata = Box::new(SearchWindowData {
            input: input.clone(),
            heroes: heroes.clone(),
            selected_hero: selected_hero.clone(),
            selected_skin: selected_skin.clone(),
            level: level.clone(),
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
    heroes: Arc<Mutex<Vec<HeroItem>>>,
    selected_hero: Arc<Mutex<usize>>,
    selected_skin: Arc<Mutex<usize>>,
    level: Arc<Mutex<MenuLevel>>,
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
            let level = data.level.lock().unwrap().clone();
            let c = char::from_u32(wparam.0 as u32).unwrap_or('\0');

            if c == '\u{1b}' {
                // Esc: 皮肤层返回英雄层，英雄层关闭
                if matches!(level, MenuLevel::Skins) {
                    *data.level.lock().unwrap() = MenuLevel::Heroes;
                    *data.selected_skin.lock().unwrap() = 0;
                    InvalidateRect(hwnd, None, true);
                } else {
                    hide_window(hwnd);
                }
                return LRESULT(0);
            }

            if c == '\r' {
                handle_enter(&data);
                return LRESULT(0);
            }

            // 只在英雄层响应文字输入
            if matches!(level, MenuLevel::Heroes) {
                if c == '\u{8}' {
                    data.input.lock().unwrap().pop();
                } else if c.is_ascii_graphic() || c == ' ' {
                    data.input.lock().unwrap().push(c);
                } else {
                    return LRESULT(0);
                }
                update_search(&data);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let data = get_data(hwnd);
            let vk = wparam.0 as u32;
            let level = data.level.lock().unwrap().clone();

            match level {
                MenuLevel::Heroes => {
                    let heroes = data.heroes.lock().unwrap();
                    let len = heroes.len();
                    let mut sel = data.selected_hero.lock().unwrap();
                    match vk {
                        VK_UP if *sel > 0 => { *sel -= 1; InvalidateRect(hwnd, None, true); }
                        VK_DOWN if *sel + 1 < len => { *sel += 1; InvalidateRect(hwnd, None, true); }
                        VK_RIGHT | VK_RETURN => { handle_enter(&data); }
                        VK_ESCAPE => { hide_window(hwnd); }
                        _ => {}
                    }
                }
                MenuLevel::Skins => {
                    let heroes = data.heroes.lock().unwrap();
                    let hero_idx = *data.selected_hero.lock().unwrap();
                    let skin_count = heroes.get(hero_idx).map(|h| h.skins.len()).unwrap_or(0);
                    let mut skin_sel = data.selected_skin.lock().unwrap();
                    match vk {
                        VK_UP if *skin_sel > 0 => { *skin_sel -= 1; InvalidateRect(hwnd, None, true); }
                        VK_DOWN if *skin_sel + 1 < skin_count => { *skin_sel += 1; InvalidateRect(hwnd, None, true); }
                        VK_LEFT | VK_ESCAPE => {
                            *data.level.lock().unwrap() = MenuLevel::Heroes;
                            *skin_sel = 0;
                            InvalidateRect(hwnd, None, true);
                        }
                        VK_RIGHT | VK_RETURN => { handle_enter(&data); }
                        _ => {}
                    }
                }
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
    *data.heroes.lock().unwrap() = build_heroes(&schemes, &input);
    *data.selected_hero.lock().unwrap() = 0;
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

fn handle_enter(data: &SearchWindowData) {
    let level = data.level.lock().unwrap().clone();
    match level {
        MenuLevel::Heroes => {
            let heroes = data.heroes.lock().unwrap();
            let sel = *data.selected_hero.lock().unwrap();
            if sel >= heroes.len() {
                return;
            }
            let hero = &heroes[sel];
            if hero.skins.len() == 1 {
                // 只有一个皮肤，直接选中
                let name = hero.skins[0].display_name.clone();
                info!("校准选择: {}", name);
                if let Some(ref cb) = *data.on_select.lock().unwrap() {
                    cb(&name);
                }
            } else {
                // 多个皮肤，展开二级菜单
                *data.level.lock().unwrap() = MenuLevel::Skins;
                *data.selected_skin.lock().unwrap() = 0;
                unsafe {
                    let hwnd = windows::Win32::UI::WindowsAndMessaging::GetWindow(
                        windows::Win32::UI::WindowsAndMessaging::GW_HWNDFIRST,
                        windows::Win32::UI::WindowsAndMessaging::GW_HWNDFIRST,
                    );
                    InvalidateRect(hwnd, None, true);
                }
            }
        }
        MenuLevel::Skins => {
            let heroes = data.heroes.lock().unwrap();
            let hero_idx = *data.selected_hero.lock().unwrap();
            let skin_idx = *data.selected_skin.lock().unwrap();
            if let Some(hero) = heroes.get(hero_idx) {
                if let Some(skin) = hero.skins.get(skin_idx) {
                    let name = skin.display_name.clone();
                    info!("校准选择: {}", name);
                    if let Some(ref cb) = *data.on_select.lock().unwrap() {
                        cb(&name);
                    }
                }
            }
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

    let level = data.level.lock().unwrap().clone();

    // ── 输入框（英雄层显示） ──
    if matches!(level, MenuLevel::Heroes) {
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
            windows::Win32::Graphics::Gdi::DT_LEFT
                | windows::Win32::Graphics::Gdi::DT_VCENTER
                | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
        );
        drop(input);
    }

    // ── 皮肤层标题栏 ──
    if matches!(level, MenuLevel::Skins) {
        let heroes = data.heroes.lock().unwrap();
        let sel = *data.selected_hero.lock().unwrap();
        let hero_name = heroes.get(sel).map(|h| h.name.clone()).unwrap_or_default();

        let header_rect = RECT {
            left: PADDING, top: PADDING,
            right: rect.right - PADDING, bottom: PADDING + INPUT_HEIGHT,
        };
        let header_bg = CreateSolidBrush(RGB(55, 55, 85));
        FillRect(hdc, &header_rect, header_bg);
        DeleteObject(header_bg);

        SetTextColor(hdc, RGB(200, 200, 240));
        let back_text: Vec<u16> = format!("← {}", hero_name).encode_utf16().collect();
        let mut bt = RECT {
            left: PADDING + 8, top: PADDING + 4,
            right: rect.right - PADDING - 8, bottom: PADDING + INPUT_HEIGHT,
        };
        windows::Win32::Graphics::Gdi::DrawTextW(
            hdc, &back_text, &mut bt,
            windows::Win32::Graphics::Gdi::DT_LEFT
                | windows::Win32::Graphics::Gdi::DT_VCENTER
                | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
        );
    }

    let list_top = PADDING + INPUT_HEIGHT + 8;
    let mut y = list_top;

    match level {
        MenuLevel::Heroes => {
            let heroes = data.heroes.lock().unwrap();
            let sel = *data.selected_hero.lock().unwrap();

            for (i, hero) in heroes.iter().enumerate() {
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

                // 英雄名
                let name: Vec<u16> = hero.name.encode_utf16().collect();
                let mut nr = RECT {
                    left: PADDING + 8, top: y + 2,
                    right: PADDING + 180, bottom: y + ROW_HEIGHT - 2,
                };
                windows::Win32::Graphics::Gdi::DrawTextW(
                    hdc, &name, &mut nr,
                    windows::Win32::Graphics::Gdi::DT_LEFT
                        | windows::Win32::Graphics::Gdi::DT_VCENTER
                        | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
                );

                // 皮肤数量标记
                if hero.skins.len() > 1 {
                    let count: Vec<u16> = format!("[{}] →", hero.skins.len())
                        .encode_utf16()
                        .collect();
                    SetTextColor(hdc, if i == sel { RGB(180, 180, 240) } else { RGB(120, 120, 150) });
                    let mut cr = RECT {
                        left: rect.right - PADDING - 80,
                        top: y + 2,
                        right: rect.right - PADDING - 8,
                        bottom: y + ROW_HEIGHT - 2,
                    };
                    windows::Win32::Graphics::Gdi::DrawTextW(
                        hdc, &count, &mut cr,
                        windows::Win32::Graphics::Gdi::DT_RIGHT
                            | windows::Win32::Graphics::Gdi::DT_VCENTER
                            | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
                    );
                }

                y += ROW_HEIGHT;
            }
        }
        MenuLevel::Skins => {
            let heroes = data.heroes.lock().unwrap();
            let hero_idx = *data.selected_hero.lock().unwrap();
            let skin_sel = *data.selected_skin.lock().unwrap();

            if let Some(hero) = heroes.get(hero_idx) {
                for (i, skin) in hero.skins.iter().enumerate() {
                    let row_rect = RECT {
                        left: PADDING + 16, top: y,
                        right: rect.right - PADDING, bottom: y + ROW_HEIGHT,
                    };

                    if i == skin_sel {
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

                    let skin_text: Vec<u16> = skin.skin_name.encode_utf16().collect();
                    let mut sr = RECT {
                        left: row_rect.left + 8,
                        top: y + 2,
                        right: row_rect.right - 8,
                        bottom: y + ROW_HEIGHT - 2,
                    };
                    windows::Win32::Graphics::Gdi::DrawTextW(
                        hdc, &skin_text, &mut sr,
                        windows::Win32::Graphics::Gdi::DT_LEFT
                            | windows::Win32::Graphics::Gdi::DT_VCENTER
                            | windows::Win32::Graphics::Gdi::DT_SINGLELINE,
                    );

                    y += ROW_HEIGHT;
                }
            }
        }
    }

    SelectObject(hdc, old_font);
    windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
}