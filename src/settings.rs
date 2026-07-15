use crate::config::AppConfig;
use eframe::egui;

/// 设置编辑器
pub struct SettingsEditor {
    cfg: AppConfig,
    dirty: bool,
}

impl SettingsEditor {
    pub fn new(cfg: AppConfig) -> Self {
        Self { cfg, dirty: false }
    }

    pub fn run(self) -> Result<(), eframe::Error> {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([550.0, 500.0])
                .with_title("300高速咏唱 - 设置"),
            ..Default::default()
        };

        eframe::run_native(
            "FasterChantSettings",
            options,
            Box::new(|_cc| Ok(Box::new(self))),
        )
    }
}

impl eframe::App for SettingsEditor {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());

        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("⚙️ 设置");
                ui.separator();
                if ui.button("💾 保存").clicked() {
                    if let Err(e) = self.cfg.save() {
                        log::error!("保存设置失败: {}", e);
                    } else {
                        self.dirty = false;
                    }
                }
                if self.dirty {
                    ui.colored_label(egui::Color32::YELLOW, "● 未保存");
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                self.render_basic(ui);
                ui.separator();
                self.render_timing(ui);
                ui.separator();
                self.render_regions(ui);
                ui.separator();
                self.render_colors(ui);
            });
        });
    }
}

impl SettingsEditor {
    fn render_basic(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("🖥️ 基本", |ui| {
            ui.horizontal(|ui| {
                ui.label("游戏窗口标题关键词:");
                if ui.text_edit_singleline(&mut self.cfg.game_window_title).changed() {
                    self.dirty = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("轮询间隔 (ms):");
                ui.add(egui::DragValue::new(&mut self.cfg.poll_interval_ms)
                    .range(100..=5000)
                    .speed(100));
                if ui.button("默认").clicked() {
                    self.cfg.poll_interval_ms = 500;
                    self.dirty = true;
                }
            });
        });
    }

    fn render_timing(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("⏱️ 骚话定时", |ui| {
            ui.horizontal(|ui| {
                ui.label("骚话间隔 (秒):");
                ui.add(egui::DragValue::new(&mut self.cfg.taunt_interval_secs)
                    .range(10..=600)
                    .speed(5));
                if ui.button("默认").clicked() {
                    self.cfg.taunt_interval_secs = 60;
                    self.dirty = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("战斗冷却 (秒):");
                ui.add(egui::DragValue::new(&mut self.cfg.taunt_cooldown_secs)
                    .range(0..=60)
                    .speed(1));
                if ui.button("默认").clicked() {
                    self.cfg.taunt_cooldown_secs = 5;
                    self.dirty = true;
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                if ui.checkbox(&mut self.cfg.burst_mode, "连发模式").changed() {
                    self.dirty = true;
                }
                if self.cfg.burst_mode {
                    ui.label("连发间隔 (ms):");
                    ui.add(egui::DragValue::new(&mut self.cfg.burst_interval_ms)
                        .range(100..=10000)
                        .speed(100));
                }
            });
        });
    }

    fn render_regions(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("📍 检测区域", |ui| {
            ui.label("所有区域使用窗口比例 (0.0 ~ 1.0)");

            self.region_editor(ui, "💎 蓝色宝石", &mut self.cfg.gem_region);
            self.region_editor(ui, "🖼️ 皮肤名 OCR", &mut self.cfg.skin_name_region);
            self.region_editor(ui, "🗺️ 小地图", &mut self.cfg.minimap_region);
            self.region_editor(ui, "⚔️ KDA 数字", &mut self.cfg.kda_region);
        });
    }

    fn region_editor(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        region: &mut crate::config::RegionRatio,
    ) {
        ui.label(label);
        ui.horizontal(|ui| {
            ui.label("x:");
            if ui.add(egui::DragValue::new(&mut region.x).range(0.0..=1.0).speed(0.01)).changed() {
                self.dirty = true;
            }
            ui.label("y:");
            if ui.add(egui::DragValue::new(&mut region.y).range(0.0..=1.0).speed(0.01)).changed() {
                self.dirty = true;
            }
            ui.label("w:");
            if ui.add(egui::DragValue::new(&mut region.w).range(0.0..=1.0).speed(0.01)).changed() {
                self.dirty = true;
            }
            ui.label("h:");
            if ui.add(egui::DragValue::new(&mut region.h).range(0.0..=1.0).speed(0.01)).changed() {
                self.dirty = true;
            }
        });
    }

    fn render_colors(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("🎨 蓝色宝石颜色范围", |ui| {
            ui.label("RGB 范围（0-255）:");
            let c = &mut self.cfg.gem_color;
            ui.horizontal(|ui| {
                ui.label("R:");
                if ui.add(egui::DragValue::new(&mut c.r_min).range(0..=255)).changed() { self.dirty = true; }
                ui.label("~");
                if ui.add(egui::DragValue::new(&mut c.r_max).range(0..=255)).changed() { self.dirty = true; }
            });
            ui.horizontal(|ui| {
                ui.label("G:");
                if ui.add(egui::DragValue::new(&mut c.g_min).range(0..=255)).changed() { self.dirty = true; }
                ui.label("~");
                if ui.add(egui::DragValue::new(&mut c.g_max).range(0..=255)).changed() { self.dirty = true; }
            });
            ui.horizontal(|ui| {
                ui.label("B:");
                if ui.add(egui::DragValue::new(&mut c.b_min).range(0..=255)).changed() { self.dirty = true; }
                ui.label("~");
                if ui.add(egui::DragValue::new(&mut c.b_max).range(0..=255)).changed() { self.dirty = true; }
            });
        });
    }
}