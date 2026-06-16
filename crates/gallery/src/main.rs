//! fluentpx 控件画廊：Win32 窗口 + DWM（深色标题栏/Mica/圆角）+ Per-Monitor v2 高 DPI
//! + 深浅主题切换 + 动画驱动，逐一陈列已实现的 Fluent 控件各状态。
//!
//! 仅 Windows 可构建（CI 在 windows-latest 上 `cargo build --release -p gallery`）。

#![windows_subsystem = "windows"]

use std::ffi::c_void;
use std::mem::size_of;
use std::time::Instant;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
const WM_MOUSELEAVE: u32 = 0x02A3;

use fluentpx::controls::{
    Button, ComboBox, ContentDialog, ListView, Slider, TabView, ToggleSwitch, ToolTip,
};
use fluentpx::gfx::{Gfx, Surface};
use fluentpx::typography::TextStyle;
use fluentpx::widget::Widget;
use fluentpx::{Dpi, EventResult, InputEvent, PaintCtx, Point, Rect, Theme};

const ANIM_TIMER: usize = 1;

/// 一个分节：标题 + 一组控件（各演示不同状态）。
struct Section {
    title: String,
    items: Vec<Box<dyn Widget>>,
    /// 段标题的逻辑 y，由 relayout 计算、paint 复用，避免两处步进漂移。
    title_y: f32,
}

struct App {
    gfx: Gfx,
    surface: Option<Surface>,
    theme: Theme,
    dpi: Dpi,
    size_px: (u32, u32),
    start: Instant,
    hwnd: HWND,
    tracking_mouse: bool,
    timer_on: bool,

    theme_btn: Button,
    theme_btn_was_pressed: bool,
    sections: Vec<Section>,
    /// 动画兜底时限：任何交互/重绘后持续刷新到此刻，保证过渡跑到静止帧。
    anim_until: f64,
}

impl App {
    fn new(gfx: Gfx) -> App {
        let theme = Theme::system();
        let sections = vec![
            Section {
                title: "Button".into(),
                title_y: 0.0,
                items: vec![
                    Box::new(Button::standard("Standard")),
                    Box::new(Button::standard("Hover me")),
                    Box::new({
                        let mut b = Button::standard("Disabled");
                        b.set_enabled(false);
                        b
                    }),
                ],
            },
            Section {
                title: "Accent Button".into(),
                title_y: 0.0,
                items: vec![
                    Box::new(Button::accent("Accent")),
                    Box::new({
                        let mut b = Button::accent("Disabled");
                        b.set_enabled(false);
                        b
                    }),
                ],
            },
            Section {
                title: "ToggleSwitch".into(),
                title_y: 0.0,
                items: vec![
                    Box::new(ToggleSwitch::new(true)),
                    Box::new(ToggleSwitch::new(false)),
                    Box::new({
                        let mut t = ToggleSwitch::new(true);
                        t.interaction.enabled = false;
                        t
                    }),
                ],
            },
            Section {
                title: "Slider".into(),
                title_y: 0.0,
                items: vec![
                    Box::new(Slider::new(0.4)),
                    Box::new({
                        let mut s = Slider::new(0.7);
                        s.interaction.enabled = false;
                        s
                    }),
                ],
            },
            Section {
                title: "ListView".into(),
                title_y: 0.0,
                items: vec![Box::new(ListView::new(
                    vec!["Apple".into(), "Banana".into(), "Cherry".into(), "Durian".into()],
                    Some(1),
                ))],
            },
            Section {
                title: "TabView".into(),
                title_y: 0.0,
                items: vec![Box::new(TabView::new(
                    vec!["Document 1".into(), "Document 2".into(), "Document 3".into()],
                    0,
                ))],
            },
            Section {
                title: "ToolTip".into(),
                title_y: 0.0,
                items: vec![Box::new(ToolTip::new("Hover me", "这是一个 ToolTip 提示气泡"))],
            },
            Section {
                title: "ComboBox".into(),
                title_y: 0.0,
                items: vec![Box::new(ComboBox::new(
                    vec!["Small".into(), "Medium".into(), "Large".into(), "Extra Large".into()],
                    1,
                ))],
            },
            Section {
                title: "ContentDialog".into(),
                title_y: 0.0,
                items: vec![Box::new(ContentDialog::new(
                    "Save your work?",
                    "你有未保存的更改，是否保存后继续？",
                ))],
            },
        ];
        App {
            gfx,
            surface: None,
            theme,
            dpi: Dpi::DEFAULT,
            size_px: (0, 0),
            start: Instant::now(),
            hwnd: HWND(std::ptr::null_mut()),
            tracking_mouse: false,
            timer_on: false,
            theme_btn: Button::standard(theme_label(theme)),
            theme_btn_was_pressed: false,
            sections,
            anim_until: 0.0,
        }
    }

    fn now(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn scale(&self) -> f32 {
        self.dpi.scale()
    }

    /// 逻辑客户区尺寸。
    fn client_logical(&self) -> (f32, f32) {
        (self.size_px.0 as f32 / self.scale(), self.size_px.1 as f32 / self.scale())
    }

    /// 重新布局：纵向分节流式排布，控件横向铺开。坐标全部逻辑像素。
    fn relayout(&mut self) {
        let (cw, _ch) = self.client_logical();
        let margin = 36.0;
        let mut y = 28.0;

        // 主题切换按钮固定右上角。
        let tb = self.theme_btn.measure(fluentpx::Size { w: cw, h: 32.0 });
        let tb_w = tb.w.max(120.0);
        self.theme_btn.arrange(Rect::new(cw - margin - tb_w, 24.0, tb_w, 32.0));

        y += 40.0; // 给页面标题留白
        for sec in &mut self.sections {
            sec.title_y = y;
            y += 36.0; // 段标题占位
            let mut x = margin;
            let row_top = y;
            let mut row_h: f32 = 0.0;
            for item in &mut sec.items {
                let want = item.measure(fluentpx::Size { w: 240.0, h: 40.0 });
                let w = want.w.max(if want.w < 60.0 { 120.0 } else { want.w });
                let h = want.h.max(32.0);
                if x + w > cw - margin && x > margin {
                    x = margin;
                    y = row_top + row_h + 16.0;
                }
                let cy = y + (40.0 - h) / 2.0;
                item.arrange(Rect::new(x, cy, w, h));
                x += w + 16.0;
                row_h = row_h.max(40.0);
            }
            y = row_top + row_h + 28.0;
        }
    }

    fn ensure_surface(&mut self) -> bool {
        if self.surface.is_none() {
            match self.gfx.create_surface(self.hwnd, self.size_px.0, self.size_px.1) {
                Ok(s) => self.surface = Some(s),
                Err(_) => return false,
            }
        }
        true
    }

    fn paint(&mut self) {
        if self.size_px.0 == 0 || self.size_px.1 == 0 {
            return;
        }
        if !self.ensure_surface() {
            return;
        }
        self.relayout();
        let tokens = self.theme.tokens();
        let scale = self.scale();
        let now = self.now();
        let dwrite = self.gfx.dwrite.clone();

        let surface = self.surface.as_mut().unwrap();
        let recreate = (|| -> Result<bool> {
            let mut painter = surface.begin(&dwrite, scale)?;
            painter.clear(tokens.solid_bg_base);

            // 页面标题
            let _ = painter.draw_text_leading(
                "fluentpx — WinUI 3 Fluent controls (Direct2D, Rust)",
                TextStyle::SUBTITLE,
                Rect::new(36.0, 20.0, 700.0, 36.0),
                tokens.text_primary,
            );

            let viewport = fluentpx::Size {
                w: self.size_px.0 as f32 / scale,
                h: self.size_px.1 as f32 / scale,
            };
            let mut ctx = PaintCtx { painter: &mut painter, tokens: &tokens, dpi: self.dpi, now, viewport };
            self.theme_btn.paint(&mut ctx);

            // 段落标题 + 控件主层（位置均由 relayout 计算）。
            for sec in &mut self.sections {
                let _ = ctx.painter.draw_text_leading(
                    &sec.title,
                    TextStyle::BODY_STRONG,
                    Rect::new(36.0, sec.title_y, 600.0, 28.0),
                    tokens.text_secondary,
                );
                for item in &mut sec.items {
                    item.paint(&mut ctx);
                }
            }

            // 覆盖层（ComboBox 下拉、ToolTip 气泡、ContentDialog 遮罩）置顶再画一遍。
            for sec in &mut self.sections {
                for item in &mut sec.items {
                    item.paint_overlay(&mut ctx);
                }
            }

            painter.end()
        })();

        match recreate {
            Ok(true) | Err(_) => {
                // 设备丢失：丢弃表面，下次重建。
                self.surface = None;
                unsafe { let _ = InvalidateRect(self.hwnd, None, false); }
            }
            Ok(false) => {}
        }
    }

    /// 把事件派发给主题按钮 + 所有控件，返回汇总结果与是否切换了主题。
    fn dispatch(&mut self, ev: InputEvent) -> EventResult {
        let now = self.now();
        let mut result = EventResult::NONE;

        // 模态捕获：若某控件处于模态（ComboBox 下拉 / ContentDialog 打开），
        // 事件只派发给它，实现焦点捕获。
        for sec in &mut self.sections {
            for item in &mut sec.items {
                if item.wants_modal() {
                    return item.on_event(ev, now);
                }
            }
        }

        self.theme_btn_was_pressed = self.theme_btn.interaction.pressed;
        result = result.or(self.theme_btn.on_event(ev, now));
        if let InputEvent::PointerUp(p) = ev {
            if self.theme_btn_was_pressed && self.theme_btn.hit_test(p) {
                self.toggle_theme();
                result.redraw = true;
            }
        }

        for sec in &mut self.sections {
            for item in &mut sec.items {
                result = result.or(item.on_event(ev, now));
            }
        }
        // 任何重绘/动画后，兜底持续刷新一小段时间，确保过渡跑到静止帧。
        if result.redraw || result.animating {
            self.anim_until = now + 0.25;
        }
        result
    }

    fn toggle_theme(&mut self) {
        self.theme = self.theme.toggled();
        self.theme_btn.text = theme_label(self.theme);
        apply_dwm_theme(self.hwnd, self.theme);
    }

    fn any_animating(&self) -> bool {
        let now = self.now();
        if self.theme_btn.is_animating(now) {
            return true;
        }
        self.sections.iter().any(|s| s.items.iter().any(|w| w.is_animating(now)))
    }

    fn update_timer(&mut self) {
        let want = self.any_animating() || self.now() < self.anim_until;
        if want && !self.timer_on {
            unsafe { SetTimer(self.hwnd, ANIM_TIMER, 16, None) };
            self.timer_on = true;
        } else if !want && self.timer_on {
            unsafe { let _ = KillTimer(self.hwnd, ANIM_TIMER); }
            self.timer_on = false;
        }
    }
}

fn theme_label(theme: Theme) -> String {
    match theme {
        Theme::Dark => "切换到浅色".into(),
        Theme::Light => "切换到深色".into(),
    }
}

/// 应用 DWM 视觉属性：深色标题栏 + Mica 背景 + 圆角。旧系统上自动忽略。
fn apply_dwm_theme(hwnd: HWND, theme: Theme) {
    unsafe {
        let dark: BOOL = theme.is_dark().into();
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const _ as *const c_void,
            size_of::<BOOL>() as u32,
        );
        let backdrop = DWMSBT_MAINWINDOW;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &backdrop as *const _ as *const c_void,
            size_of::<DWM_SYSTEMBACKDROP_TYPE>() as u32,
        );
        let corner = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const c_void,
            size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
    }
}

fn lparam_point(lp: LPARAM, scale: f32) -> Point {
    let x = (lp.0 & 0xffff) as i16 as f32;
    let y = ((lp.0 >> 16) & 0xffff) as i16 as f32;
    Point { x: x / scale, y: y / scale }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = lparam.0 as *const CREATESTRUCTW;
        let app = (*cs).lpCreateParams as *mut App;
        (*app).hwnd = hwnd;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, app as isize);
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let app_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App;
    if app_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let app = &mut *app_ptr;

    match msg {
        WM_CREATE => {
            app.dpi = Dpi::new(GetDpiForWindow(hwnd));
            apply_dwm_theme(hwnd, app.theme);
            LRESULT(0)
        }
        WM_SIZE => {
            let w = (lparam.0 & 0xffff) as u32;
            let h = ((lparam.0 >> 16) & 0xffff) as u32;
            app.size_px = (w, h);
            if let Some(s) = &app.surface {
                let _ = s.resize(w, h);
            }
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_DPICHANGED => {
            app.dpi = Dpi::new((wparam.0 & 0xffff) as u32);
            // 用消息建议的矩形重设窗口位置/大小。
            let rc = *(lparam.0 as *const RECT);
            let _ = SetWindowPos(
                hwnd,
                None,
                rc.left,
                rc.top,
                rc.right - rc.left,
                rc.bottom - rc.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            app.surface = None; // 缩放变化重建渲染资源
            let _ = InvalidateRect(hwnd, None, false);
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            if !app.tracking_mouse {
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut tme);
                app.tracking_mouse = true;
            }
            let p = lparam_point(lparam, app.scale());
            let r = app.dispatch(InputEvent::PointerMove(p));
            if r.redraw {
                let _ = InvalidateRect(hwnd, None, false);
            }
            app.update_timer();
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            app.tracking_mouse = false;
            let r = app.dispatch(InputEvent::PointerLeave);
            if r.redraw {
                let _ = InvalidateRect(hwnd, None, false);
            }
            app.update_timer();
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let _ = SetCapture(hwnd);
            let p = lparam_point(lparam, app.scale());
            let r = app.dispatch(InputEvent::PointerDown(p));
            if r.redraw {
                let _ = InvalidateRect(hwnd, None, false);
            }
            app.update_timer();
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let _ = ReleaseCapture();
            let p = lparam_point(lparam, app.scale());
            let r = app.dispatch(InputEvent::PointerUp(p));
            if r.redraw {
                let _ = InvalidateRect(hwnd, None, false);
            }
            app.update_timer();
            LRESULT(0)
        }
        WM_KEYDOWN => {
            let r = app.dispatch(InputEvent::KeyDown(wparam.0 as u32));
            if r.redraw {
                let _ = InvalidateRect(hwnd, None, false);
            }
            app.update_timer();
            LRESULT(0)
        }
        WM_TIMER => {
            let _ = InvalidateRect(hwnd, None, false);
            app.update_timer();
            LRESULT(0)
        }
        WM_SETTINGCHANGE => {
            // 跟随系统深浅色变化。
            let sys = Theme::system();
            if sys != app.theme {
                app.theme = sys;
                app.theme_btn.text = theme_label(sys);
                apply_dwm_theme(hwnd, sys);
                let _ = InvalidateRect(hwnd, None, false);
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let _ = BeginPaint(hwnd, &mut ps);
            app.paint();
            let _ = EndPaint(hwnd, &ps);
            app.update_timer();
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_DESTROY => {
            // 回收 App。
            let _ = Box::from_raw(app_ptr);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn main() -> Result<()> {
    unsafe {
        // Per-Monitor v2 高 DPI（也可由 manifest 声明）。
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        let instance = GetModuleHandleW(None)?;
        let hinstance: HINSTANCE = instance.into();
        let class_name = w!("FluentPxGalleryWindow");
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: hinstance,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            lpszClassName: class_name,
            ..Default::default()
        };
        let atom = RegisterClassExW(&wc);
        debug_assert!(atom != 0);

        let gfx = Gfx::new()?;
        let app = Box::new(App::new(gfx));
        let app_ptr = Box::into_raw(app);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("fluentpx Gallery"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            980,
            720,
            None,
            None,
            hinstance,
            Some(app_ptr as *const c_void),
        )?;

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}
