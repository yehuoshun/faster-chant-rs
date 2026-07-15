use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 英雄方案
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeroScheme {
    /// 英雄名（如 "绯村剑心"）
    pub hero_name: String,
    /// 皮肤名（可选，如 "冲田总司"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skin_name: Option<String>,
    /// 显示名（皮肤名 + 英雄名）
    pub display_name: String,
    /// 触发台词
    pub triggers: TriggerQuotes,
    /// 快捷发言面板分组
    pub panels: Vec<PanelGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerQuotes {
    /// 开局台词
    pub game_start: Vec<String>,
    /// 击杀台词
    pub kill: Vec<String>,
    /// 死亡台词
    pub death: Vec<String>,
    /// 助攻台词
    pub assist: Vec<String>,
    /// 骚话分组
    pub taunt: TauntBoxes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TauntBoxes {
    /// 骚话组，每组内随机选一条
    pub boxes: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelGroup {
    /// 分组名
    pub name: String,
    /// 发言列表
    pub lines: Vec<String>,
}

impl Default for TriggerQuotes {
    fn default() -> Self {
        Self {
            game_start: vec![],
            kill: vec![],
            death: vec![],
            assist: vec![],
            taunt: TauntBoxes { boxes: vec![] },
        }
    }
}

/// 方案管理器
#[derive(Default)]
pub struct SchemeManager {
    /// 所有方案，key = display_name
    schemes: HashMap<String, HeroScheme>,
    /// 英雄名 -> 方案 display_name 映射
    hero_index: HashMap<String, String>,
}

impl SchemeManager {
    /// 从目录加载所有方案
    pub fn load(dir: &std::path::Path) -> anyhow::Result<Self> {
        let mut manager = Self::default();
        if !dir.exists() {
            return Ok(manager);
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if entry.path().extension().map_or(false, |e| e == "json") {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    if let Ok(scheme) = serde_json::from_str::<HeroScheme>(&content) {
                        let key = scheme.display_name.clone();
                        manager.hero_index.insert(scheme.hero_name.clone(), key.clone());
                        manager.schemes.insert(key, scheme);
                    }
                }
            }
        }
        Ok(manager)
    }

    /// 匹配方案：皮肤名完整匹配 > 英雄名匹配 > 无
    pub fn match_scheme(&self, skin_name: &str, hero_name: &str) -> Option<&HeroScheme> {
        let full = format!("{} {}", skin_name, hero_name);
        // 1. 完整匹配
        if let Some(s) = self.schemes.get(&full) {
            return Some(s);
        }
        // 2. 皮肤名匹配
        if let Some(s) = self.schemes.get(skin_name) {
            return Some(s);
        }
        // 3. 英雄名匹配
        if let Some(key) = self.hero_index.get(hero_name) {
            return self.schemes.get(key);
        }
        None
    }

    /// 获取所有方案
    pub fn all(&self) -> Vec<&HeroScheme> {
        self.schemes.values().collect()
    }

    /// 保存方案
    pub fn save(&self, scheme: &HeroScheme, dir: &std::path::Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(dir)?;
        let filename = scheme
            .display_name
            .chars()
            .map(|c| if c == ' ' || c == '/' || c == '\\' { '_' } else { c })
            .collect::<String>();
        let path = dir.join(format!("{}.json", filename));
        std::fs::write(&path, serde_json::to_string_pretty(scheme)?)?;
        Ok(())
    }
}

/// 从 OCR 文本中解析皮肤名和英雄名
/// "冲田总司 绯村剑心" -> ("冲田总司", "绯村剑心")
/// "绯村剑心" -> ("绯村剑心", "绯村剑心")
pub fn parse_skin_text(text: &str) -> (String, String) {
    let text = text.trim();
    if let Some(pos) = text.rfind(' ') {
        let skin = text[..pos].trim().to_string();
        let hero = text[pos + 1..].trim().to_string();
        (skin, hero)
    } else {
        (text.to_string(), text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skin_text() {
        assert_eq!(
            parse_skin_text("冲田总司 绯村剑心"),
            ("冲田总司".to_string(), "绯村剑心".to_string())
        );
        assert_eq!(
            parse_skin_text("绯村剑心"),
            ("绯村剑心".to_string(), "绯村剑心".to_string())
        );
        assert_eq!(
            parse_skin_text("  天童木更 绯村剑心  "),
            ("天童木更".to_string(), "绯村剑心".to_string())
        );
    }
}