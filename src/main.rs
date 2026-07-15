use anyhow::Result;
use log::info;

mod config;
mod scheme;
mod window;

fn main() -> Result<()> {
    env_logger::init();
    info!("faster-chant-rs 启动");

    let cfg = config::AppConfig::load()?;
    info!("配置加载完成: {:?}", cfg);

    // 主循环: 检测确认页
    loop {
        // 1. 查找游戏窗口
        let hwnd = match window::find_game_window(&cfg.game_window_title) {
            Some(h) => h,
            None => {
                std::thread::sleep(std::time::Duration::from_millis(1000));
                continue;
            }
        };

        let rect = window::get_window_rect(hwnd);

        // 2. 检测蓝色宝石（出击锚点）
        if window::check_blue_gem(hwnd, &cfg) {
            info!("检测到确认页锚点，开始 OCR 英雄信息");

            // 3. OCR 皮肤名
            // TODO: mod ocr 下一阶段实现
            let _skin_text = "TODO: OCR result";

            // 4. 匹配方案
            // TODO: mod scheme 匹配逻辑
            info!("英雄已确认，等待进入游戏");
        }

        std::thread::sleep(std::time::Duration::from_millis(cfg.poll_interval_ms));
    }
}