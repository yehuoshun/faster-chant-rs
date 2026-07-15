use anyhow::Result;
use log::{info, warn};
use std::sync::Arc;

mod calibration;
mod config;
mod kda;
mod ocr;
mod scheme;
mod search;
mod sender;
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

    fn detect(&self, hwnd: windows::Win32::Foundation::HWND, cfg: &config::AppConfig) -> GamePage {
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
        hwnd: windows::Win32::Foundation::HWND,
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
    let schemes = Arc::new(scheme::SchemeManager::load(&cfg.schemes_dir)?);
    info!("已加载 {} 个英雄方案", schemes.all().len());

    let ocr = ocr::Ocr::new()?;
    let mut detector = PageDetector::new();
    let mut kda_tracker = kda::KdaTracker::new()?;
    let cal = Arc::new(calibration::Calibration::new());

    // 搜索弹窗（保留，后续托盘菜单触发）
    let _search_popup = search::SearchPopup::new(schemes.clone())?;

    loop {
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
                                kda::KdaEvent::Kill => {
                                    if let Some(line) = sender::pick_random(&scheme.triggers.kill) {
                                        info!("⚔️ 击杀 → {}", line);
                                        let _ = sender::send_message(line);
                                    }
                                }
                                kda::KdaEvent::Death => {
                                    if let Some(line) = sender::pick_random(&scheme.triggers.death) {
                                        info!("💀 死亡 → {}", line);
                                        let _ = sender::send_message(line);
                                    }
                                }
                                kda::KdaEvent::Assist => {
                                    if let Some(line) = sender::pick_random(&scheme.triggers.assist) {
                                        info!("🤝 助攻 → {}", line);
                                        let _ = sender::send_message(line);
                                    }
                                }
                                kda::KdaEvent::GameStart => {
                                    if let Some(line) = sender::pick_random(&scheme.triggers.game_start) {
                                        info!("🟢 开局 → {}", line);
                                        let _ = sender::send_all_chat(line);
                                    }
                                }
                                kda::KdaEvent::None => {}
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