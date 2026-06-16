//! Cloud-MGR 应用根控件：左侧导航面板 + 页面区。
//! 从原 egui 版（SidePanel + CentralPanel）迁移到 fluentpx，外观/交互对齐 WinUI。
//!
//! 已完成：外壳 + 导航 + 可用「设置」页（主题切换实际生效、下载线程）。
//! 待办(phase 2)：市场/管理页的联网拉取、下载、注册表写盘（沿用原 network/downloader/plugins）。

use fluentpx::controls::{Button, ListView, RadioButton, Slider};
use fluentpx::host::{ThemeControl, ThemeMode};
use fluentpx::typography::TextStyle;
use fluentpx::widget::*;

use crate::config::{AppConfig, ColorMode};
use crate::mode::PluginMode;

const NAV_W: f32 = 200.0;
const ROW_H: f32 = 40.0;
const IND_W: f32 = 3.0;
const IND_H: f32 = 16.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Market,
    Manage,
    Settings,
}

pub struct CloudMgrApp {
    theme: ThemeControl,
    mode: PluginMode,
    config: AppConfig,
    page: Page,
    rect: Rect,
    nav_hovered: Option<usize>,
    nav_pressed: Option<usize>,

    // 设置页控件
    theme_idx: usize, // 0=跟随系统 1=浅色 2=深色
    theme_radios: Vec<RadioButton>,
    threads: Slider,
    path_btn: Button,

    // 市场/管理页占位列表
    sample_list: ListView,
}

impl CloudMgrApp {
    pub fn new(theme: ThemeControl, mode: PluginMode) -> CloudMgrApp {
        let config = AppConfig::load().unwrap_or_default();
        // 应用已保存的主题
        let theme_idx = match config.color_mode {
            ColorMode::System => 0,
            ColorMode::Light => 1,
            ColorMode::Dark => 2,
        };
        theme.set(match theme_idx {
            1 => ThemeMode::Light,
            2 => ThemeMode::Dark,
            _ => ThemeMode::Auto,
        });

        let mut radios = vec![
            RadioButton::new("跟随系统", theme_idx == 0),
            RadioButton::new("浅色", theme_idx == 1),
            RadioButton::new("深色", theme_idx == 2),
        ];
        for (i, r) in radios.iter_mut().enumerate() {
            r.checked = i == theme_idx;
        }

        // 下载线程 1..=32 映射到 0..1
        let threads = Slider::new(((config.download_threads.clamp(1, 32) - 1) as f32) / 31.0);

        CloudMgrApp {
            theme,
            mode,
            config,
            page: Page::Market,
            rect: Rect::default(),
            nav_hovered: None,
            nav_pressed: None,
            theme_idx,
            theme_radios: radios,
            threads,
            path_btn: Button::standard("选择文件夹…"),
            sample_list: ListView::new(
                vec![
                    "示例插件 A".into(),
                    "示例插件 B".into(),
                    "示例插件 C".into(),
                    "示例插件 D".into(),
                ],
                None,
            ),
        }
    }

    fn nav_items(&self) -> [&str; 3] {
        [self.mode.get_plugin_market_name(), self.mode.get_plugin_manage_name(), "设置"]
    }

    fn nav_row_rect(&self, i: usize) -> Rect {
        let y0 = self.rect.y + 64.0;
        Rect { x: self.rect.x + 8.0, y: y0 + i as f32 * (ROW_H + 4.0), w: NAV_W - 16.0, h: ROW_H }
    }

    fn content_rect(&self) -> Rect {
        Rect { x: self.rect.x + NAV_W + 1.0, y: self.rect.y, w: (self.rect.w - NAV_W - 1.0).max(0.0), h: self.rect.h }
    }

    fn nav_index_at(&self, p: Point) -> Option<usize> {
        (0..3).find(|&i| self.nav_row_rect(i).contains(p))
    }

    fn current_threads(&self) -> u32 {
        (1.0 + self.threads.value * 31.0).round() as u32
    }

    /// 设置页控件布局。
    fn layout_settings(&mut self) {
        let c = self.content_rect();
        let x = c.x + 24.0;
        for (i, r) in self.theme_radios.iter_mut().enumerate() {
            r.arrange(Rect::new(x, c.y + 84.0 + i as f32 * 34.0, 200.0, 32.0));
        }
        self.threads.arrange(Rect::new(x, c.y + 232.0, 260.0, 22.0));
        self.path_btn.arrange(Rect::new(x, c.y + 312.0, 140.0, 32.0));
    }

    fn apply_theme_idx(&mut self, i: usize) {
        self.theme_idx = i;
        for (j, r) in self.theme_radios.iter_mut().enumerate() {
            r.checked = j == i;
        }
        let (mode, cm) = match i {
            1 => (ThemeMode::Light, ColorMode::Light),
            2 => (ThemeMode::Dark, ColorMode::Dark),
            _ => (ThemeMode::Auto, ColorMode::System),
        };
        self.theme.set(mode);
        self.config.color_mode = cm;
        let _ = self.config.save();
    }
}

impl Widget for CloudMgrApp {
    fn measure(&mut self, available: Size) -> Size {
        available
    }

    fn arrange(&mut self, rect: Rect) {
        self.rect = rect;
        self.layout_settings();
    }

    fn hit_test(&self, p: Point) -> bool {
        self.rect.contains(p)
    }

    fn paint(&mut self, ctx: &mut PaintCtx) {
        let t = ctx.tokens;

        // —— 左侧导航面板 ——
        let nav = Rect { x: self.rect.x, y: self.rect.y, w: NAV_W, h: self.rect.h };
        ctx.painter.fill_rect(nav, t.solid_bg_secondary);
        ctx.painter.fill_rect(Rect { x: self.rect.x + NAV_W, y: self.rect.y, w: 1.0, h: self.rect.h }, t.divider_stroke_default);

        // 标题
        let _ = ctx.painter.draw_text_leading(
            self.mode.get_title(),
            TextStyle::BODY_STRONG,
            Rect::new(self.rect.x + 16.0, self.rect.y + 18.0, NAV_W - 24.0, 28.0),
            t.text_primary,
        );

        let items = self.nav_items();
        let cur = match self.page {
            Page::Market => 0,
            Page::Manage => 1,
            Page::Settings => 2,
        };
        for i in 0..3 {
            let r = self.nav_row_rect(i);
            let selected = i == cur;
            let hovered = self.nav_hovered == Some(i);
            if selected {
                ctx.painter.fill_rounded_rect(r, 4.0, t.subtle_fill_secondary);
                let ind = Rect { x: r.x, y: r.center_y() - IND_H / 2.0, w: IND_W, h: IND_H };
                ctx.painter.fill_rounded_rect(ind, 1.5, t.accent_fill_default());
            } else if hovered {
                ctx.painter.fill_rounded_rect(r, 4.0, t.subtle_fill_secondary);
            }
            let fg = if selected { t.text_primary } else { t.text_secondary };
            let _ = ctx.painter.draw_text_leading(items[i], TextStyle::BODY, Rect { x: r.x + 14.0, y: r.y, w: r.w - 18.0, h: r.h }, fg);
        }

        // —— 内容区 ——
        let c = self.content_rect();
        ctx.painter.push_clip(c);
        match self.page {
            Page::Settings => self.paint_settings(ctx, c),
            Page::Market => {
                let name = self.mode.get_plugin_market_name().to_string();
                self.paint_placeholder(ctx, c, &name, "（phase 2）此处接入联网拉取插件列表、搜索、下载。下方为占位列表：");
            }
            Page::Manage => {
                let name = self.mode.get_plugin_manage_name().to_string();
                self.paint_placeholder(ctx, c, &name, "（phase 2）此处接入本地已装插件扫描、启用/禁用、卸载。下方为占位列表：");
            }
        }
        ctx.painter.pop_clip();
    }

    fn on_event(&mut self, ev: InputEvent, now: f64) -> EventResult {
        let mut r = EventResult::NONE;

        // 导航点击
        match ev {
            InputEvent::PointerMove(p) => {
                let h = self.nav_index_at(p);
                if h != self.nav_hovered {
                    self.nav_hovered = h;
                    r.redraw = true;
                }
            }
            InputEvent::PointerLeave => {
                if self.nav_hovered.is_some() {
                    self.nav_hovered = None;
                    r.redraw = true;
                }
            }
            InputEvent::PointerDown(p) => {
                self.nav_pressed = self.nav_index_at(p);
            }
            InputEvent::PointerUp(p) => {
                if let Some(i) = self.nav_index_at(p) {
                    if self.nav_pressed == Some(i) {
                        self.page = match i {
                            0 => Page::Market,
                            1 => Page::Manage,
                            _ => Page::Settings,
                        };
                        r.redraw = true;
                    }
                }
                self.nav_pressed = None;
            }
            _ => {}
        }

        // 当前页面控件
        match self.page {
            Page::Settings => {
                for radio in &mut self.theme_radios {
                    r = r.or(radio.on_event(ev, now));
                }
                // 单选互斥 + 应用主题
                if let Some(i) = (0..3).find(|&i| self.theme_radios[i].checked && i != self.theme_idx) {
                    self.apply_theme_idx(i);
                    r.redraw = true;
                } else if !self.theme_radios.iter().any(|x| x.checked) {
                    self.theme_radios[self.theme_idx].checked = true;
                }
                let tr = self.threads.on_event(ev, now);
                if tr.redraw {
                    self.config.download_threads = self.current_threads();
                    let _ = self.config.save();
                }
                r = r.or(tr);
                r = r.or(self.path_btn.on_event(ev, now));
            }
            _ => {
                r = r.or(self.sample_list.on_event(ev, now));
            }
        }
        r
    }

    fn is_animating(&self, now: f64) -> bool {
        match self.page {
            Page::Settings => {
                self.threads.is_animating(now) || self.theme_radios.iter().any(|x| x.is_animating(now))
            }
            _ => self.sample_list.is_animating(now),
        }
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::List
    }
}

impl CloudMgrApp {
    fn paint_settings(&mut self, ctx: &mut PaintCtx, c: Rect) {
        let t = ctx.tokens;
        let x = c.x + 24.0;
        let _ = ctx.painter.draw_text_leading("设置", TextStyle::TITLE, Rect::new(x, c.y + 20.0, c.w - 48.0, 40.0), t.text_primary);
        let _ = ctx.painter.draw_text_leading("外观", TextStyle::BODY_STRONG, Rect::new(x, c.y + 60.0, c.w - 48.0, 24.0), t.text_secondary);
        for radio in &mut self.theme_radios {
            radio.paint(ctx);
        }
        let _ = ctx.painter.draw_text_leading("下载线程数", TextStyle::BODY_STRONG, Rect::new(x, c.y + 200.0, c.w - 48.0, 24.0), t.text_secondary);
        self.threads.paint(ctx);
        let _ = ctx.painter.draw_text_leading(
            &format!("{} 线程", self.current_threads()),
            TextStyle::BODY,
            Rect::new(x + 276.0, c.y + 230.0, 100.0, 24.0),
            t.text_primary,
        );
        let _ = ctx.painter.draw_text_leading("默认下载路径", TextStyle::BODY_STRONG, Rect::new(x, c.y + 280.0, c.w - 48.0, 24.0), t.text_secondary);
        self.path_btn.paint(ctx);
        let path = self
            .config
            .default_download_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "（未设置，默认临时目录）".into());
        let _ = ctx.painter.draw_text_leading(&path, TextStyle::BODY, Rect::new(x + 152.0, c.y + 318.0, c.w - 200.0, 24.0), t.text_secondary);
    }

    fn paint_placeholder(&mut self, ctx: &mut PaintCtx, c: Rect, title: &str, note: &str) {
        let t = ctx.tokens;
        let x = c.x + 24.0;
        let _ = ctx.painter.draw_text_leading(title, TextStyle::TITLE, Rect::new(x, c.y + 20.0, c.w - 48.0, 40.0), t.text_primary);
        let _ = ctx.painter.draw_text_leading(note, TextStyle::BODY, Rect::new(x, c.y + 64.0, c.w - 48.0, 24.0), t.text_secondary);
        self.sample_list.arrange(Rect::new(x, c.y + 100.0, (c.w - 48.0).min(420.0), 4.0 * 40.0 + 8.0));
        self.sample_list.paint(ctx);
    }
}
