//! Cloud-MGR 应用根控件：用 fluentpx 组件搭建——
//! 左侧 **NavigationView**（可折叠）做导航，设置页用 **ComboBox** 选主题；
//! 市场/管理页用卡片列表 + 按钮。把 fluentpx 当组件库使用，而非手画外壳。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use fluentpx::controls::{ComboBox, NavItem, NavigationView, Slider, TextBox};
use fluentpx::gfx::Icon;
use fluentpx::host::{ThemeControl, ThemeMode};
use fluentpx::typography::TextStyle;
use fluentpx::widget::*;

use crate::config::{AppConfig, ColorMode};
use crate::downloader::{self, Progress};
use crate::mode::PluginMode;
use crate::plugins::{Plugin, PluginCategory, PluginManager};
use crate::utils::BootDriveManager;

const CARD_H: f32 = 76.0;
const CARD_GAP: f32 = 8.0;

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    Download(usize),
    Enable(usize),
    Disable(usize),
    Uninstall(usize),
}

enum MarketState {
    Idle,
    Loading,
    Loaded,
    Error(String),
}

pub struct CloudMgrApp {
    theme: ThemeControl,
    mode: PluginMode,
    config: AppConfig,
    rect: Rect,

    nav: NavigationView,
    last_nav: usize,

    hover: Option<Hit>,
    pressed: Option<Hit>,
    hits: Vec<(Rect, Hit)>,

    // 设置页
    theme_combo: ComboBox,
    threads: Slider,

    // 数据
    boot: BootDriveManager,
    current_drive: Option<String>,
    mgr: PluginManager,

    // 市场
    search: TextBox,
    last_search: String,
    market: MarketState,
    market_rx: Option<Receiver<Result<Vec<PluginCategory>, String>>>,
    display: Vec<Plugin>,
    scroll_market: f32,
    downloads: HashMap<String, Progress>,

    // 管理
    manage_loaded: bool,
    local: Vec<(Plugin, bool)>,
    scroll_manage: f32,
}

impl CloudMgrApp {
    pub fn new(theme: ThemeControl, mode: PluginMode) -> CloudMgrApp {
        let config = AppConfig::load().unwrap_or_default();
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

        let nav = NavigationView::shell(
            vec![
                NavItem { icon: Icon::Home, label: mode.get_plugin_market_name().to_string() },
                NavItem { icon: Icon::Folder, label: mode.get_plugin_manage_name().to_string() },
                NavItem { icon: Icon::Settings, label: "设置".to_string() },
            ],
            0,
        );

        let mut theme_combo = ComboBox::new(vec!["跟随系统".into(), "浅色".into(), "深色".into()], theme_idx);
        let _ = &mut theme_combo;
        let threads = Slider::new(((config.download_threads.clamp(1, 32) - 1) as f32) / 31.0);

        let boot = BootDriveManager::new(mode);
        let drives = boot.get_all_drives();
        let current_drive = config.default_boot_drive.clone().or_else(|| drives.first().map(|d| d.letter.clone()));

        CloudMgrApp {
            theme,
            mode,
            config,
            rect: Rect::default(),
            nav,
            last_nav: 0,
            hover: None,
            pressed: None,
            hits: Vec::new(),
            theme_combo,
            threads,
            boot,
            current_drive,
            mgr: PluginManager::new(mode),
            search: TextBox::new("搜索插件…"),
            last_search: String::new(),
            market: MarketState::Idle,
            market_rx: None,
            display: Vec::new(),
            scroll_market: 0.0,
            downloads: HashMap::new(),
            manage_loaded: false,
            local: Vec::new(),
            scroll_manage: 0.0,
        }
    }

    fn current_threads(&self) -> u32 {
        (1.0 + self.threads.value * 31.0).round() as u32
    }

    fn start_market_fetch(&mut self) {
        if matches!(self.market, MarketState::Loading) {
            return;
        }
        self.market = MarketState::Loading;
        let (tx, rx) = std::sync::mpsc::channel();
        let mode = self.mode;
        std::thread::spawn(move || {
            let _ = tx.send(crate::plugins::fetch_plugins(mode).map_err(|e| e.to_string()));
        });
        self.market_rx = Some(rx);
    }

    fn recompute_display(&mut self) {
        let kw = self.search.text.clone();
        self.display = if kw.trim().is_empty() { self.mgr.all_market_plugins() } else { self.mgr.search_plugins(&kw) };
    }

    fn load_manage(&mut self) {
        self.local.clear();
        if let Some(d) = self.current_drive.clone() {
            if self.mgr.load_local_plugins(&d).is_ok() {
                for p in self.mgr.enabled_plugins() {
                    self.local.push((p.clone(), true));
                }
                for p in self.mgr.disabled_plugins() {
                    self.local.push((p.clone(), false));
                }
            }
        }
        self.manage_loaded = true;
    }

    fn download_dir(&self) -> PathBuf {
        if let Some(d) = &self.current_drive {
            PathBuf::from(format!("{}\\{}", d, self.mode.get_plugin_folder()))
        } else if let Some(p) = &self.config.default_download_path {
            p.clone()
        } else {
            std::env::temp_dir().join("cloud-pe-plugins")
        }
    }

    fn start_download(&mut self, idx: usize) {
        if let Some(p) = self.display.get(idx).cloned() {
            if p.link.is_empty() {
                return;
            }
            let fname = p.link.rsplit('/').next().filter(|s| !s.is_empty()).map(|s| s.to_string()).unwrap_or_else(|| format!("{}.{}", p.name, self.mode.get_enabled_extension()));
            let path = self.download_dir().join(fname);
            self.downloads.insert(p.plugin_id(), downloader::spawn_download(p.link.clone(), path));
        }
    }

    fn page(&self) -> usize {
        self.nav.selected
    }

    fn on_page_switch(&mut self) {
        match self.nav.selected {
            0 if matches!(self.market, MarketState::Idle) => self.start_market_fetch(),
            1 if !self.manage_loaded => self.load_manage(),
            _ => {}
        }
        self.last_nav = self.nav.selected;
    }

    fn tick(&mut self) {
        if self.nav.selected != self.last_nav {
            self.on_page_switch();
        }
        if let Some(rx) = &self.market_rx {
            match rx.try_recv() {
                Ok(Ok(cats)) => {
                    self.mgr.set_categories(cats);
                    self.recompute_display();
                    self.market = MarketState::Loaded;
                    self.market_rx = None;
                }
                Ok(Err(e)) => {
                    self.market = MarketState::Error(e);
                    self.market_rx = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.market = MarketState::Error("拉取线程意外结束".into());
                    self.market_rx = None;
                }
            }
        }
        if self.search.text != self.last_search {
            self.last_search = self.search.text.clone();
            if matches!(self.market, MarketState::Loaded) {
                self.recompute_display();
            }
        }
        // 主题下拉变更
        let idx = self.theme_combo.selected;
        let cur = match self.config.color_mode {
            ColorMode::System => 0,
            ColorMode::Light => 1,
            ColorMode::Dark => 2,
        };
        if idx != cur {
            let (m, cm) = match idx {
                1 => (ThemeMode::Light, ColorMode::Light),
                2 => (ThemeMode::Dark, ColorMode::Dark),
                _ => (ThemeMode::Auto, ColorMode::System),
            };
            self.theme.set(m);
            self.config.color_mode = cm;
            let _ = self.config.save();
        }
    }

    fn exec_hit(&mut self, hit: Hit) {
        match hit {
            Hit::Download(i) => self.start_download(i),
            Hit::Enable(i) | Hit::Disable(i) => {
                if let (Some(d), Some((p, en))) = (self.current_drive.clone(), self.local.get(i).cloned()) {
                    let _ = if en { self.mgr.disable_plugin(&d, &p.file) } else { self.mgr.enable_plugin(&d, &p.file) };
                    self.load_manage();
                }
            }
            Hit::Uninstall(i) => {
                if let (Some(d), Some((p, _))) = (self.current_drive.clone(), self.local.get(i).cloned()) {
                    let _ = self.mgr.delete_plugin_file(&d, &p.file);
                    self.load_manage();
                }
            }
        }
    }
}

impl Widget for CloudMgrApp {
    fn measure(&mut self, available: Size) -> Size {
        available
    }
    fn arrange(&mut self, rect: Rect) {
        self.rect = rect;
        self.nav.arrange(rect);
    }
    fn hit_test(&self, p: Point) -> bool {
        self.rect.contains(p)
    }

    fn paint(&mut self, ctx: &mut PaintCtx) {
        self.tick();
        if self.page() == 0 && matches!(self.market, MarketState::Idle) {
            self.start_market_fetch();
        }
        if self.page() == 1 && !self.manage_loaded {
            self.load_manage();
        }
        self.hits.clear();

        // 左侧 NavigationView（组件，可折叠）
        self.nav.paint(ctx);
        let c = self.nav.content_area(ctx.now);

        ctx.painter.push_clip(c);
        match self.page() {
            0 => self.paint_market(ctx, c),
            1 => self.paint_manage(ctx, c),
            _ => self.paint_settings(ctx, c),
        }
        ctx.painter.pop_clip();
    }

    fn paint_overlay(&mut self, ctx: &mut PaintCtx) {
        if self.page() == 2 {
            self.theme_combo.paint_overlay(ctx);
        }
    }

    fn on_event(&mut self, ev: InputEvent, now: f64) -> EventResult {
        // 主题下拉打开时模态独占
        if self.page() == 2 && self.theme_combo.wants_modal() {
            let r = self.theme_combo.on_event(ev, now);
            return r;
        }

        let mut r = EventResult::NONE;

        // 滚轮滚动（仅内容区）
        if let InputEvent::Wheel(d) = ev {
            match self.page() {
                0 => self.scroll_market = (self.scroll_market - d * 48.0).max(0.0),
                1 => self.scroll_manage = (self.scroll_manage - d * 48.0).max(0.0),
                _ => {}
            }
            return EventResult::REDRAW;
        }

        // 导航（窗格）
        r = r.or(self.nav.on_event(ev, now));
        if self.nav.selected != self.last_nav {
            self.on_page_switch();
            r.redraw = true;
        }

        // 内容区可点击命中
        match ev {
            InputEvent::PointerMove(p) => {
                let h = self.hits.iter().find(|(rc, _)| rc.contains(p)).map(|(_, hh)| *hh);
                if h != self.hover {
                    self.hover = h;
                    r.redraw = true;
                }
            }
            InputEvent::PointerLeave => {
                if self.hover.is_some() {
                    self.hover = None;
                    r.redraw = true;
                }
            }
            InputEvent::PointerDown(p) => {
                self.pressed = self.hits.iter().find(|(rc, _)| rc.contains(p)).map(|(_, hh)| *hh);
            }
            InputEvent::PointerUp(p) => {
                if let Some(hit) = self.pressed {
                    if self.hits.iter().any(|(rc, hh)| *hh == hit && rc.contains(p)) {
                        self.exec_hit(hit);
                        r.redraw = true;
                    }
                }
                self.pressed = None;
            }
            _ => {}
        }

        // 页内嵌控件
        match self.page() {
            0 => r = r.or(self.search.on_event(ev, now)),
            2 => {
                r = r.or(self.theme_combo.on_event(ev, now));
                let tr = self.threads.on_event(ev, now);
                if tr.redraw {
                    self.config.download_threads = self.current_threads();
                    let _ = self.config.save();
                }
                r = r.or(tr);
            }
            _ => {}
        }
        r
    }

    fn is_animating(&self, now: f64) -> bool {
        let downloading = self.downloads.values().any(|p| !p.lock().map(|g| g.done).unwrap_or(true));
        self.nav.is_animating(now)
            || matches!(self.market, MarketState::Loading)
            || downloading
            || self.threads.is_animating(now)
            || self.theme_combo.is_animating(now)
            || (self.page() == 0 && self.search.is_animating(now))
    }

    fn wants_keyboard(&self) -> bool {
        self.page() == 0 && self.search.wants_keyboard()
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::List
    }
}

impl CloudMgrApp {
    fn small_button(&mut self, ctx: &mut PaintCtx, r: Rect, label: &str, hit: Hit, accent: bool) {
        let t = ctx.tokens;
        let hovered = self.hover == Some(hit);
        let pressed = self.pressed == Some(hit);
        let (bg, fg) = if accent {
            let b = if pressed { t.accent_fill_tertiary() } else if hovered { t.accent_fill_secondary() } else { t.accent_fill_default() };
            (b, t.text_on_accent_primary)
        } else {
            let b = if pressed { t.control_fill_tertiary } else if hovered { t.control_fill_secondary } else { t.control_fill_default };
            (b, t.text_primary)
        };
        ctx.painter.fill_rounded_rect(r, 4.0, bg);
        if !accent {
            ctx.painter.stroke_inner(r, 4.0, t.stroke_default, 1.0);
        }
        let _ = ctx.painter.draw_text_centered(label, TextStyle::BODY, r, fg);
        self.hits.push((r, hit));
    }

    fn paint_market(&mut self, ctx: &mut PaintCtx, c: Rect) {
        let t = ctx.tokens;
        let x = c.x + 24.0;
        let _ = ctx.painter.draw_text_leading(self.mode.get_plugin_market_name(), TextStyle::TITLE, Rect::new(x, c.y + 18.0, 300.0, 40.0), t.text_primary);
        self.search.arrange(Rect::new(c.right() - 280.0 - 24.0, c.y + 24.0, 280.0, 32.0));
        self.search.paint(ctx);

        let list_top = c.y + 76.0;
        let list = Rect { x: c.x, y: list_top, w: c.w, h: c.bottom() - list_top };
        ctx.painter.push_clip(list);
        match &self.market {
            MarketState::Loading | MarketState::Idle => {
                let _ = ctx.painter.draw_text_leading("正在拉取插件列表…", TextStyle::BODY, Rect::new(x, list_top + 20.0, 400.0, 24.0), t.text_secondary);
            }
            MarketState::Error(e) => {
                let msg = format!("加载失败：{}", e);
                let _ = ctx.painter.draw_text_leading(&msg, TextStyle::BODY, Rect::new(x, list_top + 20.0, c.w - 48.0, 48.0), t.system_critical);
            }
            MarketState::Loaded => {
                let items: Vec<Plugin> = self.display.clone();
                let mut y = list_top - self.scroll_market + 4.0;
                for (i, p) in items.iter().enumerate() {
                    let card = Rect { x, y, w: c.w - 48.0, h: CARD_H };
                    if card.bottom() > list_top && card.y < c.bottom() {
                        self.paint_card(ctx, card, p, i);
                    }
                    y += CARD_H + CARD_GAP;
                }
                if items.is_empty() {
                    let _ = ctx.painter.draw_text_leading("没有匹配的插件。", TextStyle::BODY, Rect::new(x, list_top + 20.0, 400.0, 24.0), t.text_secondary);
                }
            }
        }
        ctx.painter.pop_clip();
    }

    fn paint_card(&mut self, ctx: &mut PaintCtx, card: Rect, p: &Plugin, idx: usize) {
        let t = ctx.tokens;
        ctx.painter.fill_rounded_rect(card, 6.0, t.card_bg_default);
        ctx.painter.stroke_inner(card, 6.0, t.divider_stroke_default, 1.0);
        let _ = ctx.painter.draw_text_leading(&p.name, TextStyle::BODY_STRONG, Rect::new(card.x + 14.0, card.y + 10.0, card.w - 130.0, 22.0), t.text_primary);
        let meta = format!("v{} · {} · {}", p.version, p.author, p.size);
        let _ = ctx.painter.draw_text_leading(&meta, TextStyle::CAPTION, Rect::new(card.x + 14.0, card.y + 32.0, card.w - 130.0, 18.0), t.text_tertiary);
        if !p.describe.is_empty() {
            let _ = ctx.painter.draw_text_leading(&p.describe, TextStyle::CAPTION, Rect::new(card.x + 14.0, card.y + 50.0, card.w - 130.0, 18.0), t.text_secondary);
        }
        let btn = Rect { x: card.right() - 96.0, y: card.center_y() - 16.0, w: 84.0, h: 32.0 };
        let id = p.plugin_id();
        if let Some(prog) = self.downloads.get(&id) {
            let g = prog.lock().unwrap();
            if g.done {
                let (label, col) = if g.error.is_some() { ("失败", t.system_critical) } else { ("已完成", t.system_success) };
                let _ = ctx.painter.draw_text_centered(label, TextStyle::BODY, btn, col);
            } else {
                let pct = if g.total > 0 { (g.current as f32 / g.total as f32 * 100.0) as u32 } else { 0 };
                ctx.painter.fill_rounded_rect(Rect { x: btn.x, y: btn.center_y() - 2.0, w: btn.w, h: 4.0 }, 2.0, t.control_strong_fill_default.with_opacity(0.5));
                ctx.painter.fill_rounded_rect(Rect { x: btn.x, y: btn.center_y() - 2.0, w: btn.w * (pct as f32 / 100.0), h: 4.0 }, 2.0, t.accent_fill_default());
                let _ = ctx.painter.draw_text_centered(&format!("{}%", pct), TextStyle::CAPTION, Rect { y: btn.y - 4.0, ..btn }, t.text_secondary);
            }
        } else {
            self.small_button(ctx, btn, "下载", Hit::Download(idx), true);
        }
    }

    fn paint_manage(&mut self, ctx: &mut PaintCtx, c: Rect) {
        let t = ctx.tokens;
        let x = c.x + 24.0;
        let _ = ctx.painter.draw_text_leading(self.mode.get_plugin_manage_name(), TextStyle::TITLE, Rect::new(x, c.y + 18.0, 300.0, 40.0), t.text_primary);
        let drive_txt = match &self.current_drive {
            Some(d) => format!("启动盘：{}", d),
            None => "未检测到启动盘".to_string(),
        };
        let _ = ctx.painter.draw_text_leading(&drive_txt, TextStyle::BODY, Rect::new(c.right() - 240.0, c.y + 28.0, 220.0, 24.0), t.text_secondary);

        let list_top = c.y + 76.0;
        let list = Rect { x: c.x, y: list_top, w: c.w, h: c.bottom() - list_top };
        ctx.painter.push_clip(list);
        if self.local.is_empty() {
            let msg = if self.current_drive.is_some() { "该启动盘下暂无已安装插件。" } else { "未检测到 Cloud-PE/HotPE/Edgeless 启动盘。" };
            let _ = ctx.painter.draw_text_leading(msg, TextStyle::BODY, Rect::new(x, list_top + 20.0, c.w - 48.0, 24.0), t.text_secondary);
        } else {
            let items: Vec<(Plugin, bool)> = self.local.clone();
            let mut y = list_top - self.scroll_manage + 4.0;
            for (i, (p, en)) in items.iter().enumerate() {
                let card = Rect { x, y, w: c.w - 48.0, h: 60.0 };
                if card.bottom() > list_top && card.y < c.bottom() {
                    ctx.painter.fill_rounded_rect(card, 6.0, t.card_bg_default);
                    ctx.painter.stroke_inner(card, 6.0, t.divider_stroke_default, 1.0);
                    let _ = ctx.painter.draw_text_leading(&p.name, TextStyle::BODY_STRONG, Rect::new(card.x + 14.0, card.y + 10.0, card.w - 220.0, 22.0), t.text_primary);
                    let meta = format!("v{} · {} · {}", p.version, p.author, if *en { "已启用" } else { "已禁用" });
                    let _ = ctx.painter.draw_text_leading(&meta, TextStyle::CAPTION, Rect::new(card.x + 14.0, card.y + 32.0, card.w - 220.0, 18.0), t.text_tertiary);
                    let toggle = Rect { x: card.right() - 188.0, y: card.center_y() - 16.0, w: 84.0, h: 32.0 };
                    self.small_button(ctx, toggle, if *en { "禁用" } else { "启用" }, if *en { Hit::Disable(i) } else { Hit::Enable(i) }, false);
                    let del = Rect { x: card.right() - 96.0, y: card.center_y() - 16.0, w: 84.0, h: 32.0 };
                    self.small_button(ctx, del, "卸载", Hit::Uninstall(i), false);
                }
                y += 60.0 + CARD_GAP;
            }
        }
        ctx.painter.pop_clip();
    }

    fn paint_settings(&mut self, ctx: &mut PaintCtx, c: Rect) {
        let t = ctx.tokens;
        let x = c.x + 24.0;
        let _ = ctx.painter.draw_text_leading("设置", TextStyle::TITLE, Rect::new(x, c.y + 18.0, c.w - 48.0, 40.0), t.text_primary);
        // 外观（主题）下拉框
        let _ = ctx.painter.draw_text_leading("深浅色模式", TextStyle::BODY_STRONG, Rect::new(x, c.y + 70.0, c.w - 48.0, 24.0), t.text_secondary);
        self.theme_combo.arrange(Rect::new(x, c.y + 98.0, 220.0, 32.0));
        self.theme_combo.paint(ctx);
        // 下载线程
        let _ = ctx.painter.draw_text_leading("下载线程数", TextStyle::BODY_STRONG, Rect::new(x, c.y + 160.0, c.w - 48.0, 24.0), t.text_secondary);
        self.threads.arrange(Rect::new(x, c.y + 192.0, 260.0, 22.0));
        self.threads.paint(ctx);
        let _ = ctx.painter.draw_text_leading(&format!("{} 线程", self.current_threads()), TextStyle::BODY, Rect::new(x + 276.0, c.y + 190.0, 100.0, 24.0), t.text_primary);
    }
}
