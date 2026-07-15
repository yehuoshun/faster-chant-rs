use anyhow::Result;
use log::{debug, info, warn};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PeekMessageW,
    RegisterClassW, TranslateMessage, CW_USEDEFAULT, MSG, PM_REMOVE, WM_DESTROY, WM_HOTKEY,
    WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

mod calibration;
mod config;
mod kda;
mod ocr;
mod scheme;
mod window;

#[derive(Debug, PartialEq)]
enum GamePage {
    Confirming,
    InGame,
    Inactive,
}

struct PageDetector {
    prev: GamePage,
    current_hero: Option<String>,
    current_skin: Option<String>,
}

impl PageDetector {
    fn new() -> Self {
        Self {
            prev: GamePage::Inactive,
            current_hero: None,
            current_skin: None,
        }
    }

    fn detect(&self, hwnd: HWND, cfg: &config::AppConfig) -> GamePage {
        if window::check_blue_gem(hwnd, cfg) {
            return GamePage::Confirming;
        }
        if window::check_ingame(hwnd, cfg) {
            return GamePage::InGame;
        }
        GamePage::Inactive
    }

    fn transition(
        &mut self,
        new_page: GamePage,
        hwnd: HWND,
        cfg: &config::AppConfig,
        ocr: &ocr::Ocr,
        schemes: &scheme::SchemeManager,
        cal: &calibration::Calibration,
    ) {
        if new_page == self.prev {
            return;
        }

        match (&self.prev, &new_page) {
            (_, GamePage::Confirming) => {
                info!("→ 确认页");
                match ocr.recognize_skin_name(hwnd, cfg) {
                    Ok((skin, hero)) => {
                        info!("识别: 皮肤='{}', 英雄='{}'", skin, hero);
                        self.current_skin = Some(skin.clone());
                        self.current_hero = Some(hero.clone());
                        cal.auto_calibrate(&skin, &hero, schemes);
                    }
                    Err(e) => warn!("OCR 失败: {}", e),
                }
            }
            (GamePage::Confirming, GamePage::InGame) => info!("→ 进入游戏"),
            (GamePage::Inactive, GamePage::InGame) => info!("→ 检测到已在游戏中（冷启动）"),
            (GamePage::Confirming, GamePage::Inactive) => {
                info!("→ 离开确认页（未进入游戏）");
                self.current_hero = None;
                self.current_skin = None;
            }
            (GamePage::InGame, GamePage::Inactive) => {
                info!("→ 游戏结束/结算");
                cal.clear();
            }
            _ => {}
        }

        self.prev = new_page;
    }
}

fn main() -> Result<()> {
    env_logger::init();
    info!("faster-chant-rs 启动");

    let cfg = config::AppConfig::load()?;
    let schemes = scheme::SchemeManager::load(&cfg.schemes_dir)?;
    info!("已加载 {} 个英雄方案", schemes.all().len());

    let ocr = ocr::Ocr::new()?;
    let mut detector = PageDetector::new();
    let mut kda_tracker = kda::KdaTracker::new()?;
    let cal = Arc::new(calibration::Calibration::new());

    // 创建隐藏窗口用于接收热键消息
    let running = Arc::new(AtomicBool::new(true));
    let cal_clone = cal.clone();
    let schemes_arc = Arc::new(schemes);

    let _hotkey_thread = {
        let running = running.clone();
        let schemes = schemes_arc.clone();
        std::thread::spawn(move || {
            create_hotkey_window(running, cal_clone, schemes);
        })
    };

    // 主循环：页面检测 + KDA 追踪
    loop {
        if !running.load(Ordering::Relaxed) {
            break;
        }

        let hwnd = match window::find_game_window(&cfg.game_window_title) {
            Some(h) => h,
            None => {
                if detector.prev != GamePage::Inactive {
                    info!("游戏窗口丢失");
                    detector.prev = GamePage::Inactive;
                    detector.current_hero = None;
                    detector.current_skin = None;
                }
                std::thread::sleep(std::time::Duration::from_millis(1000));
                continue;
            }
        };

        let page = detector.detect(hwnd, &cfg);
        detector.transition(page, hwnd, &cfg, &ocr, &schemes_arc, &cal);

        if page == GamePage::InGame {
            match kda_tracker.tick(hwnd, &cfg) {
                Ok(event) => match event {
                    kda::KdaEvent::Kill => info!("⚔️ 击杀！"),
                    kda::KdaEvent::Death => info!("💀 死亡"),
                    kda::KdaEvent::Assist => info!("🤝 助攻"),
                    kda::KdaEvent::GameStart => info!("🟢 新对局开始"),
                    kda::KdaEvent::None => {}
                },
                Err(e) => warn!("KDA 检测失败: {}", e),
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(cfg.poll_interval_ms));
    }

    Ok(())
}

/// 创建隐藏窗口，处理热键消息
fn create_hotkey_window(
    running: Arc<AtomicBool>,
    cal: Arc<calibration::Calibration>,
    schemes: Arc<SchemeManager>,
) {
    unsafe {
        let hinstance = windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap();

        let class_name = windows::core::w!("FasterChantHotkeyWindow");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(hotkey_wndproc),
            hInstance: hinstance,
            lpszClassName: class_name,
            ..Default::default()
        };

        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            windows::core::w!("FasterChantHotkey"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            hinstance,
            None,
        );

        if hwnd.0 == 0 {
            warn!("创建热键窗口失败");
            return;
        }

        // 注册热键
        if let Err(e) = calibration::register_hotkey(hwnd) {
            warn!("注册热键失败: {}", e);
            return;
        }

        info!("热键窗口已创建，Ctrl+Shift+H 可校准");

        let mut msg = MSG::default();
        while running.load(Ordering::Relaxed) {
            // 非阻塞消息循环
            while PeekMessageW(&mut msg, hwnd, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_HOTKEY && msg.wParam.0 == calibration::HOTKEY_CALIBRATE as usize
                {
                    info!("Ctrl+Shift+H 触发校准");
                    // 搜索所有方案
                    let results = cal.search("", &schemes);
                    info!("可用方案 ({})：", results.len());
                    for (i, name) in results.iter().enumerate() {
                        info!("  {}. {}", i + 1, name);
                    }
                    // TODO: 弹出搜索窗口
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

unsafe extern "system" fn hotkey_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    match msg {
        WM_DESTROY => {
            windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
            windows::Win32::Foundation::LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}