use anyhow::Result;
use log::{info, warn};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

mod auto;
mod config;
mod core;
mod scheme;
mod ui;

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

    fn detect(&self, hwnd: windows::Win32::Foundation::HWND, cfg: &config::AppConfig) -> GamePage {
        if core::window::check_blue_gem(hwnd, cfg) {
            return GamePage::Confirming;
        }
        if core::window::check_ingame(hwnd, cfg) {
            return GamePage::InGame;
        }
        GamePage::Inactive
    }

    fn transition(
        &mut self,
        new_page: GamePage,
        hwnd: windows::Win32::Foundation::HWND,
        cfg: &config::AppConfig,
        ocr: &core::ocr::Ocr,
        schemes: &scheme::scheme::scheme::SchemeManager,
        cal: &auto::calibration::Calibration,
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

    // 首次启动生成默认方案
    scheme::defaults::generate_defaults(&cfg.schemes_dir)?;

    let schemes = Arc::new(scheme::scheme::scheme::scheme::SchemeManager::load(&cfg.schemes_dir)?);
    info!("已加载 {} 个英雄方案", schemes.all().len());

    let ocr = core::ocr::Ocr::new()?;
    let mut detector = PageDetector::new();
    let mut kda_tracker = core::kda::KdaTracker::new()?;
    let cal = Arc::new(auto::calibration::Calibration::new());
    let running = Arc::new(AtomicBool::new(true));

    // 系统托盘
    let tray_rx = ui::tray::Tray::spawn(running.clone())?;

    // 搜索弹窗
    let search_popup = ui::search::SearchPopup::new(schemes.clone())?;
    {
        let cal = cal.clone();
        search_popup.set_callback(move |name: &str| {
            cal.select(name);
        });
    }

    loop {
        // 处理托盘命令
        if let Ok(cmd) = tray_rx.try_recv() {
            match cmd {
                ui::tray::TrayCommand::Calibrate => {
                    search_popup.show();
                }
                ui::tray::TrayCommand::OpenEditor => {
                    info!("打开编辑器（TODO）");
                }
                ui::tray::TrayCommand::Quit => {
                    info!("退出");
                    running.store(false, Ordering::Relaxed);
                    break;
                }
            }
        }

        let hwnd = match core::window::find_game_window(&cfg.game_window_title) {
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
        detector.transition(page, hwnd, &cfg, &ocr, &schemes, &cal);

        if page == GamePage::InGame {
            match kda_tracker.tick(hwnd, &cfg) {
                Ok(event) => {
                    let scheme_name = cal.current();
                    if let Some(ref name) = scheme_name {
                        // 获取当前方案
                        let all_schemes = schemes.all();
                        if let Some(scheme) = all_schemes.iter().find(|s| s.display_name == *name) {
                            match event {
                                core::kda::KdaEvent::Kill => {
                                    if let Some(line) = auto::sender::pick_random(&scheme.triggers.kill) {
                                        info!("⚔️ 击杀 → {}", line);
                                        let _ = auto::sender::send_message(line);
                                    }
                                }
                                core::kda::KdaEvent::Death => {
                                    if let Some(line) = auto::sender::pick_random(&scheme.triggers.death) {
                                        info!("💀 死亡 → {}", line);
                                        let _ = auto::sender::send_message(line);
                                    }
                                }
                                core::kda::KdaEvent::Assist => {
                                    if let Some(line) = auto::sender::pick_random(&scheme.triggers.assist) {
                                        info!("🤝 助攻 → {}", line);
                                        let _ = auto::sender::send_message(line);
                                    }
                                }
                                core::kda::KdaEvent::GameStart => {
                                    if let Some(line) = auto::sender::pick_random(&scheme.triggers.game_start) {
                                        info!("🟢 开局 → {}", line);
                                        let _ = auto::sender::send_all_chat(line);
                                    }
                                }
                                core::kda::KdaEvent::None => {}
                            }
                        }
                    }
                }
                Err(e) => warn!("KDA 检测失败: {}", e),
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(cfg.poll_interval_ms));
    }
}