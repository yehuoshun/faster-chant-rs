use crate::config::AppConfig;
use crate::ocr::Ocr;
use crate::window;
use anyhow::Result;
use log::{debug, info};
use windows::Win32::Foundation::HWND;

/// KDA 追踪器：定期 OCR KDA 区域，检测变化触发事件
pub struct KdaTracker {
    /// 上一次的 KDA 值
    prev: Option<Kda>,
    /// OCR 引擎引用
    ocr: Ocr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Kda {
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KdaEvent {
    /// 开局（KDA 归零或首次检测）
    GameStart,
    /// 击杀 +1
    Kill,
    /// 死亡 +1
    Death,
    /// 助攻 +1
    Assist,
    /// 无变化
    None,
}

impl KdaTracker {
    pub fn new() -> Result<Self> {
        let ocr = Ocr::new()?;
        Ok(Self { prev: None, ocr })
    }

    /// 检测一帧 KDA，返回事件
    pub fn tick(&mut self, hwnd: HWND, cfg: &AppConfig) -> Result<KdaEvent> {
        let text = self.ocr.recognize_region(hwnd, &cfg.kda_region)?;
        debug!("KDA OCR 原始文本: {:?}", text);

        let current = match parse_kda(&text) {
            Some(k) => k,
            None => {
                debug!("KDA 解析失败，跳过");
                return Ok(KdaEvent::None);
            }
        };

        debug!("KDA 解析: {:?}", current);

        let prev = match &self.prev {
            Some(p) => p.clone(),
            None => {
                // 首次检测
                self.prev = Some(current);
                if current.kills == 0 && current.deaths == 0 && current.assists == 0 {
                    return Ok(KdaEvent::GameStart);
                }
                return Ok(KdaEvent::None);
            }
        };

        // 检测变化
        let event = if current.kills > prev.kills {
            KdaEvent::Kill
        } else if current.deaths > prev.deaths {
            KdaEvent::Death
        } else if current.assists > prev.assists {
            KdaEvent::Assist
        } else if current.kills == 0 && current.deaths == 0 && current.assists == 0
            && (prev.kills > 0 || prev.deaths > 0 || prev.assists > 0)
        {
            // KDA 归零 = 新一局
            KdaEvent::GameStart
        } else {
            KdaEvent::None
        };

        self.prev = Some(current);
        Ok(event)
    }
}

/// 从 OCR 文本中解析 KDA
/// 支持格式：
/// - "0 / 0 / 1"
/// - "0/0/1"
/// - "11 / 7 / 5"（死亡计数模式）
/// - 杂乱的 OCR 文本中包含 KDA 数字
fn parse_kda(text: &str) -> Option<Kda> {
    // 提取所有数字
    let numbers: Vec<u32> = text
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse::<u32>().ok())
        .collect();

    // 需要至少 3 个数字
    if numbers.len() < 3 {
        return None;
    }

    // 尝试找到连续的 K/D/A 三元组
    // KDA 值通常不会太大（< 100），用这个过滤掉金币/时间等大数字
    for window in numbers.windows(3) {
        let (k, d, a) = (window[0], window[1], window[2]);
        if k <= 99 && d <= 99 && a <= 99 {
            return Some(Kda {
                kills: k,
                deaths: d,
                assists: a,
            });
        }
    }

    // 如果所有的三元组都超过 99，取第一个（可能是死亡计数模式的大数字）
    if let Some(window) = numbers.windows(3).next() {
        return Some(Kda {
            kills: window[0],
            deaths: window[1],
            assists: window[2],
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_kda_standard() {
        let kda = parse_kda("0 / 0 / 1").unwrap();
        assert_eq!(kda.kills, 0);
        assert_eq!(kda.deaths, 0);
        assert_eq!(kda.assists, 1);
    }

    #[test]
    fn test_parse_kda_no_spaces() {
        let kda = parse_kda("5/2/8").unwrap();
        assert_eq!(kda.kills, 5);
        assert_eq!(kda.deaths, 2);
        assert_eq!(kda.assists, 8);
    }

    #[test]
    fn test_parse_kda_with_noise() {
        let kda = parse_kda("PING:46 FPS:179 11 / 7 / 5 2728/1007").unwrap();
        // 应该匹配到 11/7/5（都 <= 99）
        assert_eq!(kda.kills, 11);
        assert_eq!(kda.deaths, 7);
        assert_eq!(kda.assists, 5);
    }

    #[test]
    fn test_parse_kda_death_count_mode() {
        // 死亡计数模式：数字较大
        let kda = parse_kda("2728 / 1007 / 11 / 26").unwrap();
        // 取第一个三元组
        assert_eq!(kda.kills, 2728);
        assert_eq!(kda.deaths, 1007);
        assert_eq!(kda.assists, 11);
    }

    #[test]
    fn test_parse_kda_empty() {
        assert!(parse_kda("").is_none());
        assert!(parse_kda("PING:46 FPS:179").is_none());
    }
}