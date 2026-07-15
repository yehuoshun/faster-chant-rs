use crate::scheme::scheme::{HeroScheme, PanelGroup, TauntBoxes, TriggerQuotes};
use eframe::egui;
use std::path::PathBuf;

/// 英雄方案编辑器
pub struct HeroEditor {
    /// 所有方案
    schemes: Vec<HeroScheme>,
    /// 当前选中的方案索引
    selected: usize,
    /// 方案目录
    dir: PathBuf,
    /// 是否有未保存的修改
    dirty: bool,
    /// 搜索框文本
    search: String,
}

impl HeroEditor {
    pub fn new(schemes: Vec<HeroScheme>, dir: PathBuf) -> Self {
        Self {
            schemes,
            selected: 0,
            dir,
            dirty: false,
            search: String::new(),
        }
    }

    /// 运行编辑器窗口
    pub fn run(self) -> Result<(), eframe::Error> {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([900.0, 650.0])
                .with_title("300高速咏唱 - 英雄编辑器"),
            ..Default::default()
        };

        eframe::run_native(
            "FasterChantEditor",
            options,
            Box::new(|_cc| Ok(Box::new(self))),
        )
    }
}

impl eframe::App for HeroEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 暗色主题
        ctx.set_visuals(egui::Visuals::dark());

        self.render_top_bar(ctx);
        self.render_body(ctx);
    }
}

impl HeroEditor {
    fn render_top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🦀 英雄方案编辑器");
                ui.separator();
                if ui.button("💾 保存").clicked() {
                    self.save_current();
                }
                if self.dirty {
                    ui.colored_label(egui::Color32::YELLOW, "● 未保存");
                }
            });
        });
    }

    fn render_body(&mut self, ctx: &egui::Context) {
        // 左侧：方案列表
        egui::SidePanel::left("scheme_list")
            .resizable(false)
            .default_width(180.0)
            .show(ctx, |ui| {
                self.render_scheme_list(ui);
            });

        // 右侧：编辑区
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.schemes.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label("暂无方案，确认页识别到新英雄后自动创建");
                });
                return;
            }
            self.render_editor(ui);
        });
    }

    fn render_scheme_list(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("🔍");
            ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("搜索..."));
        });
        ui.separator();

        let search = self.search.to_lowercase();
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut to_select = None;
            for (i, scheme) in self.schemes.iter().enumerate() {
                if !search.is_empty()
                    && !scheme.hero_name.to_lowercase().contains(&search)
                    && !scheme.display_name.to_lowercase().contains(&search)
                {
                    continue;
                }

                let is_selected = i == self.selected;
                let resp = ui.selectable_label(is_selected, &scheme.hero_name);

                // 皮肤名
                if let Some(ref skin) = scheme.skin_name {
                    ui.label(egui::RichText::new(skin).size(11.0).color(egui::Color32::GRAY));
                }

                if resp.clicked() {
                    to_select = Some(i);
                }
            }

            if let Some(i) = to_select {
                self.selected = i;
            }

            ui.separator();
            if ui.button("+ 新建方案").clicked() {
                let new_scheme = HeroScheme {
                    hero_name: "新英雄".into(),
                    skin_name: None,
                    display_name: "新英雄".into(),
                    triggers: TriggerQuotes::default(),
                    panels: vec![PanelGroup {
                        name: "集合".into(),
                        lines: vec![],
                    }],
                };
                self.schemes.push(new_scheme);
                self.selected = self.schemes.len() - 1;
                self.dirty = true;
            }
        });
    }

    fn render_editor(&mut self, ui: &mut egui::Ui) {
        let Some(scheme) = self.schemes.get_mut(self.selected) else {
            return;
        };

        egui::ScrollArea::vertical().show(ui, |ui| {
            // 基本信息
            ui.collapsing("📋 基本信息", |ui| {
                ui.horizontal(|ui| {
                    ui.label("英雄名:");
                    if ui.text_edit_singleline(&mut scheme.hero_name).changed() {
                        self.dirty = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("皮肤名:");
                    let mut skin = scheme.skin_name.clone().unwrap_or_default();
                    if ui.text_edit_singleline(&mut skin).changed() {
                        scheme.skin_name = if skin.is_empty() { None } else { Some(skin) };
                        self.dirty = true;
                    }
                });
                scheme.display_name = format!(
                    "{}",
                    scheme
                        .skin_name
                        .as_ref()
                        .map(|s| format!("{} {}", s, scheme.hero_name))
                        .unwrap_or_else(|| scheme.hero_name.clone())
                );
                ui.label(format!("显示名: {}", scheme.display_name));
            });

            ui.separator();

            // 触发台词
            ui.collapsing("🎯 触发台词", |ui| {
                self.render_trigger_section(ui, "🟢 开局", &mut scheme.triggers.game_start);
                self.render_trigger_section(ui, "⚔️ 击杀", &mut scheme.triggers.kill);
                self.render_trigger_section(ui, "💀 死亡", &mut scheme.triggers.death);
                self.render_trigger_section(ui, "🤝 助攻", &mut scheme.triggers.assist);
            });

            ui.separator();

            // 骚话
            ui.collapsing("🗣️ 骚话分组", |ui| {
                let mut remove_box = None;
                for (i, box_lines) in scheme.triggers.taunt.boxes.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("分组 {}:", i + 1));
                        if ui.button("❌").clicked() {
                            remove_box = Some(i);
                        }
                    });
                    self.render_text_lines(ui, box_lines);
                    ui.separator();
                }
                if let Some(i) = remove_box {
                    scheme.triggers.taunt.boxes.remove(i);
                    self.dirty = true;
                }
                if ui.button("+ 添加分组").clicked() {
                    scheme.triggers.taunt.boxes.push(vec![String::new()]);
                    self.dirty = true;
                }
            });

            ui.separator();

            // 快捷面板
            ui.collapsing("⌨️ 快捷面板", |ui| {
                let mut remove_panel = None;
                for (i, panel) in scheme.panels.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label("面板名:");
                        if ui.text_edit_singleline(&mut panel.name).changed() {
                            self.dirty = true;
                        }
                        if ui.button("❌").clicked() {
                            remove_panel = Some(i);
                        }
                    });
                    self.render_text_lines(ui, &mut panel.lines);
                    ui.separator();
                }
                if let Some(i) = remove_panel {
                    scheme.panels.remove(i);
                    self.dirty = true;
                }
                if ui.button("+ 添加面板").clicked() {
                    scheme.panels.push(PanelGroup {
                        name: "新面板".into(),
                        lines: vec![],
                    });
                    self.dirty = true;
                }
            });
        });
    }

    fn render_trigger_section(&mut self, ui: &mut egui::Ui, label: &str, lines: &mut Vec<String>) {
        ui.horizontal(|ui| {
            ui.label(label);
            ui.label(format!("({} 条)", lines.len()));
        });
        self.render_text_lines(ui, lines);
    }

    /// 渲染文本行列表（每行一个输入框）
    fn render_text_lines(&mut self, ui: &mut egui::Ui, lines: &mut Vec<String>) {
        let mut remove_idx = None;
        for (i, line) in lines.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{}.", i + 1));
                if ui.text_edit_singleline(line).changed() {
                    self.dirty = true;
                }
                if ui.button("✕").clicked() {
                    remove_idx = Some(i);
                }
            });
        }
        if let Some(i) = remove_idx {
            lines.remove(i);
            self.dirty = true;
        }
        if ui.button("+ 添加").clicked() {
            lines.push(String::new());
            self.dirty = true;
        }
    }

    fn save_current(&mut self) {
        if let Some(scheme) = self.schemes.get(self.selected) {
            let filename = scheme
                .display_name
                .chars()
                .map(|c| if c == ' ' || c == '/' || c == '\\' { '_' } else { c })
                .collect::<String>();
            let path = self.dir.join(format!("{}.json", filename));
            if let Ok(json) = serde_json::to_string_pretty(scheme) {
                if std::fs::write(&path, json).is_ok() {
                    self.dirty = false;
                    log::info!("方案已保存: {}", scheme.display_name);
                }
            }
        }
    }
}