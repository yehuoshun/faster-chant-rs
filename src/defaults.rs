use crate::scheme::{HeroScheme, PanelGroup, TauntBoxes, TriggerQuotes};
use std::path::Path;

/// 内置默认台词库（空模板，用户自行填充）
pub fn builtin_schemes() -> Vec<HeroScheme> {
    vec![
        HeroScheme {
            hero_name: "绯村剑心".into(),
            skin_name: None,
            display_name: "绯村剑心".into(),
            triggers: TriggerQuotes::default(),
            panels: vec![
                PanelGroup { name: "集合".into(), lines: vec![] },
                PanelGroup { name: "撤退".into(), lines: vec![] },
            ],
        },
        HeroScheme {
            hero_name: "卫宫".into(),
            skin_name: None,
            display_name: "卫宫".into(),
            triggers: TriggerQuotes::default(),
            panels: vec![
                PanelGroup { name: "集合".into(), lines: vec![] },
                PanelGroup { name: "撤退".into(), lines: vec![] },
            ],
        },
        HeroScheme {
            hero_name: "桐人".into(),
            skin_name: None,
            display_name: "桐人".into(),
            triggers: TriggerQuotes::default(),
            panels: vec![
                PanelGroup { name: "集合".into(), lines: vec![] },
                PanelGroup { name: "撤退".into(), lines: vec![] },
            ],
        },
    ]
}

/// 生成默认方案文件到目录
pub fn generate_defaults(dir: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    for scheme in builtin_schemes() {
        let filename = scheme
            .display_name
            .chars()
            .map(|c| if c == ' ' || c == '/' || c == '\\' { '_' } else { c })
            .collect::<String>();
        let path = dir.join(format!("{}.json", filename));
        if !path.exists() {
            let json = serde_json::to_string_pretty(&scheme)?;
            std::fs::write(&path, json)?;
            log::info!("生成默认方案: {}", scheme.display_name);
        }
    }
    Ok(())
}