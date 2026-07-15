# 系统托盘设计

## 菜单结构

```
🦀 300高速咏唱 (左键单击恢复窗口)
─────────────────────────
当前英雄: 绯村剑心 (冲田总司)   ← 灰色，不可点击
状态: 游戏中                    ← 灰色
─────────────────────────
切换英雄 (Ctrl+Shift+H)  →
  ├── 绯村剑心
  ├── 卫宫
  ├── 桐人
  └── ... (动态加载方案列表)
英雄编辑器               → 打开 egui 编辑窗口
设置                     → 打开设置窗口
─────────────────────────
调试模式    ✓ 开/关      ← 勾选
─────────────────────────
退出
```

## 托盘图标状态

| 图标 | 含义 |
|------|------|
| 🦀 正常 | 等待中（主菜单/大厅） |
| 🟢 绿点 | 游戏中，KDA 追踪激活 |
| 🟡 黄点 | 确认页，英雄已识别 |
| 🔴 红点 | 错误/游戏窗口丢失 |

(用不同颜色的 tray icon 或 overlay 小圆点表示)

## 代码结构

```rust
// src/tray.rs

use tray_icon::{TrayIconBuilder, menu::MenuBuilder};
use std::sync::Arc;

pub struct TrayManager {
    icon: tray_icon::TrayIcon,
    // 通道：从托盘菜单发送命令到主循环
    tx: crossbeam::channel::Sender<TrayCommand>,
}

pub enum TrayCommand {
    SwitchHero(String),    // 手动切换英雄
    OpenEditor,            // 打开编辑器
    OpenSettings,          // 打开设置
    ToggleDebug,           // 切换调试模式
    Quit,                  // 退出
}

impl TrayManager {
    pub fn new(tx: Sender<TrayCommand>) -> Self {
        let icon = load_icon(IconState::Normal); // 加载 ICO/PNG

        let menu = MenuBuilder::new()
            .text("当前英雄: 未检测", false) // disabled
            .separator()
            .text("切换英雄", true)
            .text("英雄编辑器", true)
            .text("设置", true)
            .separator()
            .checkbox("调试模式", false, true)
            .separator()
            .text("退出", true)
            .build();

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .with_tooltip("300高速咏唱")
            .on_menu_event(move |event| {
                let cmd = match event.id.as_ref() {
                    "切换英雄" => TrayCommand::SwitchHero,
                    "英雄编辑器" => TrayCommand::OpenEditor,
                    "设置" => TrayCommand::OpenSettings,
                    "调试模式" => TrayCommand::ToggleDebug,
                    "退出" => TrayCommand::Quit,
                    _ => return,
                };
                tx.send(cmd).ok();
            })
            .build()
            .unwrap();

        Self { icon: tray }
    }

    /// 更新托盘状态
    pub fn set_state(&mut self, page: GamePage, hero: Option<&str>) {
        let icon = match page {
            GamePage::InGame => load_icon(IconState::InGame),
            GamePage::Confirming => load_icon(IconState::Confirming),
            GamePage::Inactive => load_icon(IconState::Normal),
        };
        self.icon.set_icon(icon).ok();

        let tooltip = match hero {
            Some(h) => format!("300高速咏唱 - {}", h),
            None => "300高速咏唱".to_string(),
        };
        self.icon.set_tooltip(Some(tooltip)).ok();
    }
}
```

## 主循环集成

```rust
fn main() {
    // ... 初始化 ...

    let (tx, rx) = crossbeam::unbounded::<TrayCommand>();
    let tray = TrayManager::new(tx);

    loop {
        // 处理托盘命令（非阻塞）
        if let Ok(cmd) = rx.try_recv() {
            match cmd {
                SwitchHero(name) => { /* 切换方案 */ }
                OpenEditor => { /* 打开 egui 窗口 */ }
                OpenSettings => { /* 打开设置窗口 */ }
                ToggleDebug => { /* 切换调试模式 */ }
                Quit => break,
            }
        }

        // 检测页面 + KDA 追踪（已有逻辑）
        let page = detector.detect(hwnd, &cfg);
        detector.transition(page, ...);

        // 更新托盘状态
        tray.set_state(page, detector.current_hero.as_deref());

        sleep(cfg.poll_interval_ms);
    }
}
```

## 不需要的东西

- ❌ 悬浮窗 — 不做，纯热键 + 托盘
- ❌ 托盘气泡通知 — 刷屏烦人，不要
- ❌ 复杂动画 — 托盘图标只换颜色，不搞花活