//! Cloud-MGR 应用根控件：左侧导航 + 市场/管理/设置三页（从原 egui 版迁移到 fluentpx）。
//!
//! 市场页：后台线程阻塞拉取插件列表 → 卡片列表 + 搜索 + 下载（进度共享回传）。
//! 管理页：扫描启动盘本地插件 → 启用/禁用（改扩展名）+ 卸载。
//! 设置页：主题切换实际生效 + 下载线程 + 默认下载路径。
//! 异步与 Win32 单线程宿主集成：后台线程 + mpsc/共享进度，UI 每帧轮询。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};

use fluentpx::controls::{RadioButton, Slider, TextBox};
use fluentpx::host::{ThemeControl, ThemeMode};
use fluentpx::typography::TextStyle;
use fluentpx::widget::*;
use fluentpx::Color;

use crate::config::{AppConfig, ColorMode};
use crate::downloader::{self, Progress};
use crate::mode::PluginMode;
use crate::plugins::{Plugin, PluginCategory, PluginManager};
use crate::utils::BootDriveManager;

const NAV_W: f32 = 200.0;
const ROW_H: f32 = 40.0;
const IND_W: f32 = 3.0;
const IND_H: f32 = 16.0;
const CARD_H: f32 = 76.0;
const CARD_GAP: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Market,
    Manage,
    Settings,
}

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    Nav(usize),
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
    page: Page,
    rect: Rect,

    hover: Option<Hit>,
    pressed: Option<Hit>,
    hits: Vec<(Rect, Hit)>,

    // 设置页
    theme_idx: usize,
    theme_radios: Vec<RadioButton>,
    threads: Slider,

    // 数据
    boot: BootDriveManager,
    current_drive: Option<String>,
    mgr: PluginManager,

    // 市场页
    search: TextBox,
    last_search: String,
    market: MarketState,
    market_rx: Option<Receiver<Result<Vec<PluginCategory>, String>>>,
    display: Vec<Plugin>,
    scroll_market: f32,
    downloads: HashMap<String, Progress>,

    // 管理页
    manage_loaded: bool,
    local: Vec<(Plugin, bool)>, // (plugin, enabled)
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
        let mut radios = vec![
            RadioButton::new("跟随系统", theme_idx == 0),
            RadioButton::new("浅色", theme_idx == 1),
            RadioButton::new("深色", theme_idx == 2),
        ];
        for (i, r) in radios.iter_mut().enumerate() {
            r.checked = i == theme_idx;
        }
        let threads = Slider::new(((config.download_threads.clamp(1, 32) - 1) as f32) / 31.0);

        let boot = BootDriveManager::new(mode);
        let drives = boot.get_all_drives();
        let current_drive = config
            .default_boot_drive
            .clone()
            .or_else(|| drives.first().map(|d| d.letter.clone()));

        CloudMgrApp {
            theme,
            mode,
            config,
            page: Page::Market,
            rect: Rect::default(),
            hover: None,
            pressed: None,
            hits: Vec::new(),
            theme_idx,
            theme_radios: radios,
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
            let r = crate::plugins::fetch_plugins(mode).map_err(|e| e.to_string());
            let _ = tx.send(r);
        });
        self.market_rx = Some(rx);
    }

    fn recompute_display(&mut self) {
        let kw = self.search.text.clone();
        self.display = if kw.trim().is_empty() {
            self.mgr.all_market_plugins()
        } else {
            self.mgr.search_plugins(&kw)
        };
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
            let prog = downloader::spawn_download(p.link.clone(), path);
            self.downloads.insert(p.plugin_id(), prog);
        }
    }

    /// 每帧轮询：拉取结果 + 下载进度。
    fn tick(&mut self) {
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
    }

    fn apply_theme_idx(&mut self, i: usize) {
        self.theme_idx = i;
        for (j, r) in self.theme_radios.iter_mut().enumerate() {
            r.checked = j == i;
        }
        let (m, cm) = match i {
            1 => (ThemeMode::Light, ColorMode::Light),
            2 => (ThemeMode::Dark, ColorMode::Dark),
            _ => (ThemeMode::Auto, ColorMode::System),
        };
        self.theme.set(m);
        self.config.color_mode = cm;
        let _ = self.config.save();
    }

    fn layout_settings(&mut self) {
        let c = self.content_rect();
        let x = c.x + 24.0;
        for (i, r) in self.theme_radios.iter_mut().enumerate() {
            r.arrange(Rect::new(x, c.y + 84.0 + i as f32 * 34.0, 200.0, 32.0));
        }
        self.threads.arrange(Rect::new(x, c.y + 232.0, 260.0, 22.0));
    }

    fn exec_hit(&mut self, hit: Hit) {
        match hit {
            Hit::Nav(i) => {
                self.page = match i {
                    0 => Page::Market,
                    1 => Page::Manage,
                    _ => Page::Settings,
                };
                if self.page == Page::Market && matches!(self.market, MarketState::Idle) {
                    self.start_market_fetch();
                }
                if self.page == Page::Manage && !self.manage_loaded {
                    self.load_manage();
                }
            }
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
        self.layout_settings();
    }
    fn hit_test(&self, p: Point) -> bool {
        self.rect.contains(p)
    }

    fn paint(&mut self, ctx: &mut PaintCtx) {
        self.tick();
        if self.page == Page::Market && matches!(self.market, MarketState::Idle) {
            self.start_market_fetch();
        }
        if self.page == Page::Manage && !self.manage_loaded {
            self.load_manage();
        }
        self.hits.clear();
        let t = ctx.tokens;

        // 左侧导航
        let nav = Rect { x: self.rect.x, y: self.rect.y, w: NAV_W, h: self.rect.h };
        ctx.painter.fill_rect(nav, t.solid_bg_secondary);
        ctx.painter.fill_rect(Rect { x: self.rect.x + NAV_W, y: self.rect.y, w: 1.0, h: self.rect.h }, t.divider_stroke_default);
        let _ = ctx.painter.draw_text_leading(self.mode.get_title(), TextStyle::BODY_STRONG, Rect::new(self.rect.x + 16.0, self.rect.y + 18.0, NAV_W - 24.0, 28.0), t.text_primary);
        let items: [String; 3] = {
            let it = self.nav_items();
            [it[0].to_string(), it[1].to_string(), it[2].to_string()]
        };
        let cur = match self.page {
            Page::Market => 0,
            Page::Manage => 1,
            Page::Settings => 2,
        };
        for i in 0..3 {
            let r = self.nav_row_rect(i);
            self.hits.push((r, Hit::Nav(i)));
            let selected = i == cur;
            if selected || self.hover == Some(Hit::Nav(i)) {
                ctx.painter.fill_rounded_rect(r, 4.0, t.subtle_fill_secondary);
            }
            if selected {
                ctx.painter.fill_rounded_rect(Rect { x: r.x, y: r.center_y() - IND_H / 2.0, w: IND_W, h: IND_H }, 1.5, t.accent_fill_default());
            }
            let fg = if selected { t.text_primary } else { t.text_secondary };
            let _ = ctx.painter.draw_text_leading(&items[i], TextStyle::BODY, Rect { x: r.x + 14.0, y: r.y, w: r.w - 18.0, h: r.h }, fg);
        }

        // 内容区
        let c = self.content_rect();
        ctx.painter.push_clip(c);
        match self.page {
            Page::Market => self.paint_market(ctx, c),
            Page::Manage => self.paint_manage(ctx, c),
            Page::Settings => self.paint_settings(ctx, c),
        }
        ctx.painter.pop_clip();
    }

    fn on_event(&mut self, ev: InputEvent, now: f64) -> EventResult {
        let mut r = EventResult::NONE;
        // 滚动
        if let InputEvent::Wheel(d) = ev {
            match self.page {
                Page::Market => self.scroll_market = (self.scroll_market - d * 48.0).max(0.0),
                Page::Manage => self.scroll_manage = (self.scroll_manage - d * 48.0).max(0.0),
                _ => {}
            }
            return EventResult::REDRAW;
        }
        // 通用可点击命中
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
        // 页面内嵌控件
        match self.page {
            Page::Market => {
                r = r.or(self.search.on_event(ev, now));
            }
            Page::Settings => {
                for radio in &mut self.theme_radios {
                    r = r.or(radio.on_event(ev, now));
                }
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
            }
            _ => {}
        }
        r
    }

    fn is_animating(&self, now: f64) -> bool {
        let downloading = self.downloads.values().any(|p| !p.lock().map(|g| g.done).unwrap_or(true));
        matches!(self.market, MarketState::Loading)
            || downloading
            || self.threads.is_animating(now)
            || self.theme_radios.iter().any(|x| x.is_animating(now))
            || (self.page == Page::Market && self.search.is_animating(now))
    }

    fn wants_keyboard(&self) -> bool {
        self.page == Page::Market && self.search.wants_keyboard()
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
        // 搜索框
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
                let n = self.display.len();
                let mut y = list_top - self.scroll_market + 4.0;
                // 收集要画的项（避免借用冲突）
                let items: Vec<Plugin> = self.display.clone();
                for (i, p) in items.iter().enumerate() {
                    let card = Rect { x: x, y, w: c.w - 48.0, h: CARD_H };
                    if card.bottom() > list_top && card.y < c.bottom() {
                        self.paint_card(ctx, card, p, i);
                    }
                    y += CARD_H + CARD_GAP;
                }
                if n == 0 {
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
        // 下载按钮 / 进度
        let btn = Rect { x: card.right() - 96.0, y: card.center_y() - 16.0, w: 84.0, h: 32.0 };
        let id = p.plugin_id();
        if let Some(prog) = self.downloads.get(&id) {
            let g = prog.lock().unwrap();
            if g.done {
                let label = if g.error.is_some() { "失败" } else { "已完成" };
                let _ = ctx.painter.draw_text_centered(label, TextStyle::BODY, btn, if g.error.is_some() { t.system_critical } else { t.system_success });
            } else {
                let pct = if g.total > 0 { (g.current as f32 / g.total as f32 * 100.0) as u32 } else { 0 };
                // 进度条
                ctx.painter.fill_rounded_rect(Rect { x: btn.x, y: btn.center_y() - 2.0, w: btn.w, h: 4.0 }, 2.0, t.control_strong_fill_default.with_opacity(0.5));
                let fw = btn.w * (pct as f32 / 100.0);
                ctx.painter.fill_rounded_rect(Rect { x: btn.x, y: btn.center_y() - 2.0, w: fw, h: 4.0 }, 2.0, t.accent_fill_default());
                let _ = ctx.painter.draw_text_centered(&format!("{}%", pct), TextStyle::CAPTION, Rect { y: btn.y - 4.0, ..btn }, t.text_secondary);
            }
        } else {
            self.small_button(ctx, btn, "下载", Hit::Download(idx), true);
        }
        let _ = Color::TRANSPARENT;
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
        let _ = ctx.painter.draw_text_leading("外观", TextStyle::BODY_STRONG, Rect::new(x, c.y + 60.0, c.w - 48.0, 24.0), t.text_secondary);
        for radio in &mut self.theme_radios {
            radio.paint(ctx);
        }
        let _ = ctx.painter.draw_text_leading("下载线程数", TextStyle::BODY_STRONG, Rect::new(x, c.y + 200.0, c.w - 48.0, 24.0), t.text_secondary);
        self.threads.paint(ctx);
        let _ = ctx.painter.draw_text_leading(&format!("{} 线程", self.current_threads()), TextStyle::BODY, Rect::new(x + 276.0, c.y + 230.0, 100.0, 24.0), t.text_primary);
    }
}
