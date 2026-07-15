use anyhow::Result;
use log::{debug, info, warn};

mod config;
mod ocr;
mod scheme;
mod window;

fn main() -> Result<()> {
    env_logger::init();
    info!("faster-chant-rs 启动");

    let cfg = config::AppConfig::load()?;
    info!("配置加载完成");

    let schemes = scheme::SchemeManager::load(&cfg.schemes_dir)?;
    info!("已加载 {} 个英雄方案", schemes.all().len());

    let ocr = ocr::Ocr::new()?;

    // 状态机：跟踪确认页状态
    let mut was_on_confirm = false;

    loop {
        let hwnd = match window::find_game_window(&cfg.game_window_title) {
            Some(h) => h,
            None => {
                if was_on_confirm {
                    info!("游戏窗口丢失，可能已退出");
                    was_on_confirm = false;
                }
                std::thread::sleep(std::time::Duration::from_millis(1000));
                continue;
            }
        };

        let gem_detected = window::check_blue_gem(hwnd, &cfg);

        if gem_detected && !was_on_confirm {
            // 刚进入确认页：检测到蓝色宝石
            info!("检测到确认页锚点（蓝色宝石），识别英雄...");

            match ocr.recognize_skin_name(hwnd, &cfg) {
                Ok((skin, hero)) => {
                    info!("识别结果: 皮肤='{}', 英雄='{}'", skin, hero);

                    match schemes.match_scheme(&skin, &hero) {
                        Some(scheme) => {
                            info!("匹配到方案: {} ({} 条触发台词)",
                                scheme.display_name,
                                scheme.triggers.kill.len() + scheme.triggers.death.len());
                        }
                        None => {
                            info!("未找到方案，创建新方案: {} {}", skin, hero);
                            // TODO: 自动创建默认方案
                        }
                    }
                }
                Err(e) => {
                    warn!("皮肤名 OCR 失败: {}", e);
                }
            }
            was_on_confirm = true;
        } else if !gem_detected && was_on_confirm {
            // 蓝色宝石消失：进入游戏了
            info!("确认页关闭，进入游戏模式");
            // TODO: 启动 KDA OCR 检测
            was_on_confirm = false;
        }

        std::thread::sleep(std::time::Duration::from_millis(cfg.poll_interval_ms));
    }
}