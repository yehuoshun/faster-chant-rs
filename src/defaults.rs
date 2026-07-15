use crate::scheme::{HeroScheme, PanelGroup, TauntBoxes, TriggerQuotes};
use std::path::Path;

/// 内置默认台词库
/// 首次启动时自动生成 JSON 文件，用户可自由修改
pub fn builtin_schemes() -> Vec<HeroScheme> {
    vec![
        // ── 绯村剑心 ──
        HeroScheme {
            hero_name: "绯村剑心".into(),
            skin_name: None,
            display_name: "绯村剑心".into(),
            triggers: TriggerQuotes {
                game_start: vec![
                    "剑是凶器，剑术是杀人之术。".into(),
                    "飞天御剑流，参上。".into(),
                ],
                kill: vec![
                    "你已经死了。".into(),
                    "不过如此。".into(),
                    "下一个。".into(),
                ],
                death: vec![
                    "我的我的...".into(),
                    "剑，断了。".into(),
                ],
                assist: vec![
                    "好配合。".into(),
                    "干得漂亮。".into(),
                ],
                taunt: TauntBoxes {
                    boxes: vec![
                        vec!["你搁这刮痧呢？".into(), "就这？".into()],
                        vec!["吾之生涯一片无悔。".into(), "这便是我的全部了。".into()],
                    ],
                },
            },
            panels: vec![
                PanelGroup {
                    name: "集合".into(),
                    lines: vec!["来这里".into(), "集合打团".into(), "别分散".into(), "跟上".into()],
                },
                PanelGroup {
                    name: "撤退".into(),
                    lines: vec!["撤！".into(), "别打了快跑".into(), "回防".into()],
                },
            ],
        },
        // ── 卫宫 ──
        HeroScheme {
            hero_name: "卫宫".into(),
            skin_name: None,
            display_name: "卫宫".into(),
            triggers: TriggerQuotes {
                game_start: vec![
                    "I am the bone of my sword.".into(),
                    "开始吧。".into(),
                ],
                kill: vec![
                    "Trace on。".into(),
                    "结束了。".into(),
                ],
                death: vec![
                    "身体...是剑骨头做的。".into(),
                    "咳...".into(),
                ],
                assist: vec![
                    "投影完毕。".into(),
                ],
                taunt: TauntBoxes {
                    boxes: vec![
                        vec!["你就这点本事？".into()],
                        vec!["无限剑制！".into()],
                    ],
                },
            },
            panels: vec![
                PanelGroup {
                    name: "集合".into(),
                    lines: vec!["集合".into(), "打团".into()],
                },
                PanelGroup {
                    name: "撤退".into(),
                    lines: vec!["撤退".into()],
                },
            ],
        },
        // ── 桐人 ──
        HeroScheme {
            hero_name: "桐人".into(),
            skin_name: None,
            display_name: "桐人".into(),
            triggers: TriggerQuotes {
                game_start: vec![
                    "Link Start！".into(),
                    "要上了。".into(),
                ],
                kill: vec![
                    "星爆气流斩！".into(),
                    "切换。".into(),
                ],
                death: vec![
                    "HP 归零...".into(),
                ],
                assist: vec![
                    "切换得好。".into(),
                ],
                taunt: TauntBoxes {
                    boxes: vec![
                        vec!["你太慢了。".into()],
                        vec!["这可不是游戏。".into()],
                    ],
                },
            },
            panels: vec![
                PanelGroup {
                    name: "集合".into(),
                    lines: vec!["集合".into()],
                },
                PanelGroup {
                    name: "撤退".into(),
                    lines: vec!["撤退".into()],
                },
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