# faster-chant-rs 🦀

> 300英雄快捷喊话工具 Rust 重制版  
> 基于 [FasterChantDevice (Anran-233)](https://github.com/Anran-233/FasterChantDevice) 理念重写，融合 [yehuoshun/FasterChantDevice](https://github.com/yehuoshun/FasterChantDevice) 2.0 的 OCR 事件检测能力

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021%20edition-orange.svg)](https://www.rust-lang.org/)

## 🚧 开发中

本项目处于早期开发阶段，计划功能：

- [ ] 全局键盘钩子（`WH_KEYBOARD_LL`）
- [ ] 悬浮穿透面板（`WS_EX_TRANSPARENT` + `WS_EX_LAYERED`）
- [ ] 英雄方案管理（JSON 格式，支持二级面板）
- [ ] 拼音搜索切换方案
- [ ] 连发模式
- [ ] OCR 事件检测（K/D/A 自动触发）
- [ ] egui 编辑器/设置界面
- [ ] 系统托盘
- [ ] 调试模式

## 🛠 技术栈

- Rust 2021 edition
- `windows` crate — Win32 API 直接调用
- `egui` — 即时模式 GUI 编辑器
- `serde` + `serde_json` — 配置与方案序列化
- Windows.Media.Ocr — 内置 OCR 引擎

## 🙏 致谢

- [FasterChantDevice (Anran-233)](https://github.com/Anran-233/FasterChantDevice) — 原始理念与 C++/Qt 实现
- [yehuoshun/FasterChantDevice](https://github.com/yehuoshun/FasterChantDevice) — C# .NET 8 重制版，OCR 事件检测灵感来源