use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 游戏窗口标题关键词
    pub game_window_title: String,
    /// 确认页轮询间隔（毫秒）
    pub poll_interval_ms: u64,
    /// 蓝色宝石锚点区域（窗口比例，0.0~1.0）
    pub gem_region: RegionRatio,
    /// 蓝色宝石目标颜色 RGB 范围
    pub gem_color: ColorRange,
    /// 皮肤名 OCR 区域（窗口比例）
    pub skin_name_region: RegionRatio,
    /// 游戏内小地图锚点区域（窗口比例）
    pub minimap_region: RegionRatio,
    /// 英雄方案目录
    pub schemes_dir: PathBuf,
    /// 全局设置文件路径
    pub settings_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionRatio {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorRange {
    pub r_min: u8,
    pub r_max: u8,
    pub g_min: u8,
    pub g_max: u8,
    pub b_min: u8,
    pub b_max: u8,
}

impl Default for AppConfig {
    fn default() -> Self {
        let base = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("faster-chant-rs");

        Self {
            game_window_title: "300英雄".to_string(),
            poll_interval_ms: 500,
            gem_region: RegionRatio {
                // 蓝色宝石在底部中央，粗略估计
                x: 0.42,
                y: 0.88,
                w: 0.16,
                h: 0.04,
            },
            gem_color: ColorRange {
                // 亮蓝色范围，待精确校准
                r_min: 0,
                r_max: 80,
                g_min: 100,
                g_max: 200,
                b_min: 180,
                b_max: 255,
            },
            skin_name_region: RegionRatio {
                // 皮肤名在右下皮肤面板
                x: 0.55,
                y: 0.62,
                w: 0.30,
                h: 0.06,
            },
            minimap_region: RegionRatio {
                // 小地图：右下角，彩色圆点 + 地形
                x: 0.78,
                y: 0.78,
                w: 0.20,
                h: 0.20,
            },
            schemes_dir: base.join("schemes"),
            settings_path: base.join("settings.json"),
        }
    }
}

impl AppConfig {
    pub fn load() -> Result<Self, anyhow::Error> {
        let path = Self::default().settings_path;
        if path.exists() {
            let data = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&data)?)
        } else {
            let cfg = Self::default();
            // 确保目录存在
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::create_dir_all(&cfg.schemes_dir)?;
            // 写入默认配置
            std::fs::write(&path, serde_json::to_string_pretty(&cfg)?)?;
            Ok(cfg)
        }
    }
}