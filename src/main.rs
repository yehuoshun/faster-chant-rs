use anyhow::Result;
use log::{debug, info, warn};
use windows::Win32::Foundation::HWND;

mod config;
mod ocr;
mod scheme;
mod window;

/// 游戏页面状态
#[derive(Debug, PartialEq)]
enum GamePage {
    /// 确认页：英雄已选，等待出击
    Confirming,
    /// 游戏中：KDA 可见，喊话逻辑激活
    InGame,
    /// 结算/主菜单/大厅/未启动
    Inactive,
}

/// 状态机：检测当前页面
struct PageDetector {
    /// 上一次状态
    prev: GamePage,
    /// 当前选中的英雄名（确认页捕获）
    current_hero: Option<String>,
    /// 当前皮肤名
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

    /// 检测当前页面
    fn detect(&self, hwnd: HWND, cfg: &config::AppConfig) -> GamePage {
        // 1. 确认页：蓝色宝石存在
        if window::check_blue_gem(hwnd, cfg) {
            return GamePage::Confirming;
        }

        // 2. 游戏中：KDA 区域有数字（TODO：等截图后实现）
        // if window::check_kda(hwnd, cfg) {
        //     return GamePage::InGame;
        // }

        // 3. 都不是 → 结算/主菜单/大厅
        GamePage::Inactive
    }

    /// 处理状态转换
    fn transition(&mut self, new_page: GamePage, hwnd: HWND, cfg: &config::AppConfig, ocr: &ocr::Ocr, schemes: &scheme::SchemeManager) {
        if new_page == self.prev {
            return; // 状态未变，跳过
        }

        match (&self.prev, &new_page) {
            // 进入确认页
            (_, GamePage::Confirming) => {
                info!("→ 确认页");
                match ocr.recognize_skin_name(hwnd, cfg) {
                    Ok((skin, hero)) => {
                        info!("识别: 皮肤='{}', 英雄='{}'", skin, hero);
                        self.current_skin = Some(skin.clone());
                        self.current_hero = Some(hero.clone());

                        match schemes.match_scheme(&skin, &hero) {
                            Some(s) => info!("方案匹配: {}", s.display_name),
                            None => info!("未找到方案: {} {}", skin, hero),
                        }
                    }
                    Err(e) => warn!("OCR 失败: {}", e),
                }
            }
            // 离开确认页 → 进入游戏
            (GamePage::Confirming, GamePage::InGame) => {
                info!("→ 进入游戏，启动 KDA 检测");
                // TODO: 启动 KDA OCR 循环
            }
            // 离开确认页 → 非活跃（取消确认/异常）
            (GamePage::Confirming, GamePage::Inactive) => {
                info!("→ 离开确认页（未进入游戏）");
                self.current_hero = None;
                self.current_skin = None;
            }
            // 游戏结束 → 非活跃
            (GamePage::InGame, GamePage::Inactive) => {
                info!("→ 游戏结束/结算");
                // TODO: 停止 KDA 检测
            }
            // 从非活跃进入游戏（冷启动时已在游戏中）
            (GamePage::Inactive, GamePage::InGame) => {
                info!("→ 检测到已在游戏中（冷启动）");
                // TODO: 启动 KDA 检测 + OCR 英雄名兜底
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
    info!("配置加载完成");

    let schemes = scheme::SchemeManager::load(&cfg.schemes_dir)?;
    info!("已加载 {} 个英雄方案", schemes.all().len());

    let ocr = ocr::Ocr::new()?;
    let mut detector = PageDetector::new();

    loop {
        let hwnd = match window::find_game_window(&cfg.game_window_title) {
            Some(h) => h,
            None => {
                // 游戏窗口不存在
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
        detector.transition(page, hwnd, &cfg, &ocr, &schemes);

        std::thread::sleep(std::time::Duration::from_millis(cfg.poll_interval_ms));
    }
}