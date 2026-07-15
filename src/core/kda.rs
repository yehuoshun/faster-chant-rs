use crate::config::AppConfig;
use crate::core::ocr::Ocr;
use crate::core::window;
use anyhow::Result;
use log::{debug, info};
use windows::Win32::Foundation::HWND;

/// KDA 追踪器
pub struct KdaTracker {
    prev: Option<Kda>,
    ocr: Ocr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Kda {
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub cs: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KdaEvent {
    GameStart,
    Kill,
    Death,
    Assist,
    None,
}

impl KdaTracker {
    pub fn new() -> Result<Self> {
        let ocr = Ocr::new()?;
        Ok(Self { prev: None, ocr })
    }

    pub fn tick(&mut self, hwnd: HWND, cfg: &AppConfig) -> Result<KdaEvent> {
        let text = self.ocr.recognize_region(hwnd, &cfg.kda_region)?;
        debug!("KDA OCR 原始: {:?}", text);

        let current = match parse_kda(&text) {
            Some(k) => k,
            None => {
                debug!("KDA 解析失败");
                return Ok(KdaEvent::None);
            }
        };

        debug!("KDA: {:?}", current);

        let prev = match &self.prev {
            Some(p) => p.clone(),
            None => {
                self.prev = Some(current);
                if current.kills == 0 && current.deaths == 0
                    && current.assists == 0 && current.cs == 0
                {
                    return Ok(KdaEvent::GameStart);
                }
                return Ok(KdaEvent::None);
            }
        };

        let event = if current.kills > prev.kills {
            KdaEvent::Kill
        } else if current.deaths > prev.deaths {
            KdaEvent::Death
        } else if current.assists > prev.assists {
            KdaEvent::Assist
        } else if current.kills == 0 && current.deaths == 0
            && current.assists == 0 && current.cs == 0
            && (prev.kills > 0 || prev.deaths > 0 || prev.assists > 0)
        {
            KdaEvent::GameStart
        } else {
            KdaEvent::None
        };

        self.prev = Some(current);
        Ok(event)
    }
}

/// 解析 KDA 文本
///
/// 标准模式（3组）：击杀 / 助攻 / 补刀
///   例: "0 / 0 / 1" → kills=0, deaths=0, assists=0, cs=1
///
/// 死亡计数模式（4组）：击杀 / 死亡 / 助攻 / 补刀
///   例: "11 / 7 / 5 / 26" → kills=11, deaths=7, assists=5, cs=26
fn parse_kda(text: &str) -> Option<Kda> {
    let numbers: Vec<u32> = text
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse::<u32>().ok())
        .collect();

    if numbers.len() < 3 {
        return None;
    }

    // 找 3-4 个连续的小数字（KDA 值通常 < 100，补刀 < 500）
    for window in numbers.windows(4) {
        if window[0] <= 99 && window[1] <= 99 && window[2] <= 99 && window[3] <= 999 {
            // 死亡计数模式
            return Some(Kda {
                kills: window[0],
                deaths: window[1],
                assists: window[2],
                cs: window[3],
            });
        }
    }

    for window in numbers.windows(3) {
        if window[0] <= 99 && window[1] <= 99 && window[2] <= 999 {
            // 标准模式：击杀 助攻 补刀，没有死亡
            return Some(Kda {
                kills: window[0],
                deaths: 0, // 标准模式不显示死亡
                assists: window[1],
                cs: window[2],
            });
        }
    }

    // 兜底：取前 3-4 个数字
    if numbers.len() >= 4 {
        Some(Kda {
            kills: numbers[0],
            deaths: numbers[1],
            assists: numbers[2],
            cs: numbers[3],
        })
    } else {
        Some(Kda {
            kills: numbers[0],
            deaths: 0,
            assists: numbers[1],
            cs: numbers[2],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_mode() {
        let kda = parse_kda("0 / 0 / 1").unwrap();
        assert_eq!(kda.kills, 0);
        assert_eq!(kda.deaths, 0); // 标准模式无死亡
        assert_eq!(kda.assists, 0);
        assert_eq!(kda.cs, 1);
    }

    #[test]
    fn test_death_count_mode() {
        let kda = parse_kda("11 / 7 / 5 / 26").unwrap();
        assert_eq!(kda.kills, 11);
        assert_eq!(kda.deaths, 7);
        assert_eq!(kda.assists, 5);
        assert_eq!(kda.cs, 26);
    }

    #[test]
    fn test_death_count_with_noise() {
        let kda = parse_kda("PING:46 11 / 7 / 5 / 26 2728/1007").unwrap();
        assert_eq!(kda.kills, 11);
        assert_eq!(kda.deaths, 7);
        assert_eq!(kda.assists, 5);
        assert_eq!(kda.cs, 26);
    }

    #[test]
    fn test_standard_with_noise() {
        let kda = parse_kda("FPS:60 5 / 2 / 8 PING:46").unwrap();
        assert_eq!(kda.kills, 5);
        assert_eq!(kda.deaths, 0);
        assert_eq!(kda.assists, 2);
        assert_eq!(kda.cs, 8);
    }

    #[test]
    fn test_empty() {
        assert!(parse_kda("").is_none());
        assert!(parse_kda("PING:46 FPS:179").is_none());
    }
}