//! TextBox（编辑框，单行）：焦点 / 光标 / 选区 / **右键上下文菜单**（剪切·复制·粘贴·全选）。
//!
//! 真值参考：`controls/dev/CommonStyles/TextBox_themeresources.xaml`
//! * MinHeight 32，圆角 4，背景 `ControlFillColorDefault`（聚焦 `ControlFillColorInputActive`）
//! * 边框 `ControlElevationBorderBrush`；聚焦底部 2px `SystemAccentColor` 焦点下划线
//! * 选区高亮用 `AccentFillColorSelectedTextBackground`（此处用 accent 近似）
//!
//! 限制：仅处理 WM_CHAR 与基本编辑键，**中文 IME 需对接 TSF**；鼠标拖选暂未做（与拖动滚动冲突）。

use crate::clipboard;
use crate::typography::TextStyle;
use crate::widget::*;

const BOX_H: f32 = 32.0;
const CORNER: f32 = 4.0;
const BORDER: f32 = 1.0;
const PAD_L: f32 = 11.0;
const FOCUS_LINE: f32 = 2.0;
const BLINK_MS: f64 = 530.0;
const MENU_W: f32 = 150.0;
const MENU_ITEM_H: f32 = 32.0;
const MENU_VPAD: f32 = 4.0;

const MENU_ITEMS: [&str; 4] = ["剪切", "复制", "粘贴", "全选"];

pub struct TextBox {
    pub text: String,
    pub placeholder: String,
    caret: usize,
    sel_anchor: Option<usize>, // 选区另一端；None 表示无选区
    focused: bool,
    hovered: bool,
    rect: Rect,
    pub enabled: bool,
    focus_time: f64,
    // 右键菜单
    ctx_open: bool,
    ctx_pos: Point,
    ctx_hover: Option<usize>,
}

impl TextBox {
    pub fn new(placeholder: impl Into<String>) -> TextBox {
        TextBox {
            text: String::new(),
            placeholder: placeholder.into(),
            caret: 0,
            sel_anchor: None,
            focused: false,
            hovered: false,
            rect: Rect::default(),
            enabled: true,
            focus_time: 0.0,
            ctx_open: false,
            ctx_pos: Point::default(),
            ctx_hover: None,
        }
    }

    fn char_len(&self) -> usize {
        self.text.chars().count()
    }

    fn caret_visible(&self, now: f64) -> bool {
        if !self.focused || self.sel_range().is_some() {
            return false;
        }
        let elapsed_ms = (now - self.focus_time) * 1000.0;
        ((elapsed_ms / BLINK_MS) as i64) % 2 == 0
    }

    /// 归一化选区（起,止），无选区返回 None。
    fn sel_range(&self) -> Option<(usize, usize)> {
        let a = self.sel_anchor?;
        if a == self.caret {
            None
        } else {
            Some((a.min(self.caret), a.max(self.caret)))
        }
    }

    fn selected_text(&self) -> String {
        match self.sel_range() {
            Some((s, e)) => self.text.chars().skip(s).take(e - s).collect(),
            None => String::new(),
        }
    }

    /// 删除当前选区（若有），caret 落到选区起点。返回是否删除了内容。
    fn delete_selection(&mut self) -> bool {
        if let Some((s, e)) = self.sel_range() {
            let bs = byte_index(&self.text, s);
            let be = byte_index(&self.text, e);
            self.text.replace_range(bs..be, "");
            self.caret = s;
            self.sel_anchor = None;
            true
        } else {
            false
        }
    }

    fn insert_str(&mut self, ins: &str) {
        self.delete_selection();
        let idx = byte_index(&self.text, self.caret);
        self.text.insert_str(idx, ins);
        self.caret += ins.chars().count();
        self.sel_anchor = None;
    }

    fn do_action(&mut self, action: usize) {
        match action {
            0 => {
                // 剪切
                let t = self.selected_text();
                if !t.is_empty() {
                    clipboard::set_text(&t);
                    self.delete_selection();
                }
            }
            1 => {
                // 复制
                let t = self.selected_text();
                if !t.is_empty() {
                    clipboard::set_text(&t);
                }
            }
            2 => {
                // 粘贴
                if let Some(s) = clipboard::get_text() {
                    let s: String = s.chars().filter(|c| !c.is_control()).collect();
                    self.insert_str(&s);
                }
            }
            3 => {
                // 全选
                if self.char_len() > 0 {
                    self.sel_anchor = Some(0);
                    self.caret = self.char_len();
                }
            }
            _ => {}
        }
    }

    fn menu_rect(&self) -> Rect {
        let h = MENU_ITEMS.len() as f32 * MENU_ITEM_H + MENU_VPAD * 2.0;
        Rect { x: self.ctx_pos.x, y: self.ctx_pos.y, w: MENU_W, h }
    }

    fn menu_item_rect(&self, i: usize) -> Rect {
        let m = self.menu_rect();
        Rect { x: m.x, y: m.y + MENU_VPAD + i as f32 * MENU_ITEM_H, w: m.w, h: MENU_ITEM_H }
    }
}

impl Widget for TextBox {
    fn measure(&mut self, available: Size) -> Size {
        Size { w: available.w.clamp(220.0, 320.0), h: BOX_H }
    }

    fn arrange(&mut self, rect: Rect) {
        self.rect = Rect { x: rect.x, y: rect.y, w: rect.w, h: BOX_H };
    }

    fn hit_test(&self, p: Point) -> bool {
        self.rect.contains(p) || (self.ctx_open && self.menu_rect().contains(p))
    }

    fn paint(&mut self, ctx: &mut PaintCtx) {
        let t = ctx.tokens;
        let r = self.rect;

        let bg = if !self.enabled {
            t.control_fill_disabled
        } else if self.focused {
            t.control_fill_input_active
        } else if self.hovered {
            t.control_fill_secondary
        } else {
            t.control_fill_default
        };
        ctx.painter.fill_rounded_rect(r.inset(BORDER), (CORNER - BORDER).max(0.0), bg);
        let _ = ctx.painter.stroke_inner_gradient(r, CORNER, &t.control_elevation_border(), BORDER);

        let inner = Rect { x: r.x + PAD_L, y: r.y, w: r.w - PAD_L * 2.0, h: r.h };

        // 选区高亮
        if let Some((s, e)) = self.sel_range() {
            let pre: String = self.text.chars().take(s).collect();
            let mid: String = self.text.chars().skip(s).take(e - s).collect();
            let x0 = inner.x + ctx.painter.measure_text(&pre, TextStyle::BODY).map(|z| z.w).unwrap_or(0.0);
            let w = ctx.painter.measure_text(&mid, TextStyle::BODY).map(|z| z.w).unwrap_or(0.0);
            let hl = Rect { x: x0, y: r.y + 5.0, w, h: r.h - 10.0 };
            ctx.painter.fill_rounded_rect(hl, 2.0, t.accent_fill_default().with_opacity(0.5));
        }

        // 文字 / 占位符
        if self.text.is_empty() && !self.focused {
            let _ = ctx.painter.draw_text_leading(&self.placeholder, TextStyle::BODY, inner, t.text_tertiary);
        } else {
            let fg = if self.enabled { t.text_primary } else { t.text_disabled };
            let _ = ctx.painter.draw_text_leading(&self.text, TextStyle::BODY, inner, fg);
        }

        // 光标
        if self.caret_visible(ctx.now) {
            let prefix: String = self.text.chars().take(self.caret).collect();
            let w = ctx.painter.measure_text(&prefix, TextStyle::BODY).map(|s| s.w).unwrap_or(0.0);
            let cx = inner.x + w;
            ctx.painter.draw_line(cx, r.y + 7.0, cx, r.bottom() - 7.0, t.text_primary, 1.0);
        }

        // 聚焦底部 accent 下划线
        if self.focused {
            let line = Rect { x: r.x, y: r.bottom() - FOCUS_LINE, w: r.w, h: FOCUS_LINE };
            ctx.painter.fill_rounded_rect(line, FOCUS_LINE / 2.0, t.accent_fill_default());
        }
    }

    fn paint_overlay(&mut self, ctx: &mut PaintCtx) {
        if !self.ctx_open {
            return;
        }
        let t = ctx.tokens;
        let m = self.menu_rect();
        ctx.painter.fill_rounded_rect(Rect { y: m.y + 2.0, ..m }, 8.0, crate::color::Color::hex("#30000000"));
        ctx.painter.fill_rounded_rect(m, 8.0, t.solid_bg_tertiary);
        ctx.painter.stroke_inner(m, 8.0, t.surface_stroke_flyout, 1.0);
        for i in 0..MENU_ITEMS.len() {
            let ir = self.menu_item_rect(i);
            // 「剪切/复制」无选区时禁用观感，「粘贴」无剪贴板内容时也淡化（简化：仅按选区判断剪切/复制）。
            let enabled = match i {
                0 | 1 => self.sel_range().is_some(),
                _ => true,
            };
            if self.ctx_hover == Some(i) && enabled {
                ctx.painter.fill_rounded_rect(Rect { x: ir.x + 4.0, y: ir.y + 1.0, w: ir.w - 8.0, h: ir.h - 2.0 }, 4.0, t.subtle_fill_secondary);
            }
            let fg = if enabled { t.text_primary } else { t.text_disabled };
            let tr = Rect { x: ir.x + 12.0, y: ir.y, w: ir.w - 20.0, h: ir.h };
            let _ = ctx.painter.draw_text_leading(MENU_ITEMS[i], TextStyle::BODY, tr, fg);
        }
    }

    fn on_event(&mut self, ev: InputEvent, now: f64) -> EventResult {
        if !self.enabled {
            return EventResult::NONE;
        }
        // 右键菜单打开时优先处理
        if self.ctx_open {
            match ev {
                InputEvent::PointerMove(p) => {
                    let h = (0..MENU_ITEMS.len()).find(|&i| self.menu_item_rect(i).contains(p));
                    if h != self.ctx_hover {
                        self.ctx_hover = h;
                        return EventResult::REDRAW;
                    }
                    return EventResult::NONE;
                }
                InputEvent::PointerDown(p) => {
                    if !self.menu_rect().contains(p) {
                        self.ctx_open = false;
                        return EventResult::REDRAW;
                    }
                    return EventResult::NONE;
                }
                InputEvent::PointerUp(p) => {
                    if let Some(i) = (0..MENU_ITEMS.len()).find(|&i| self.menu_item_rect(i).contains(p)) {
                        let enabled = matches!(i, 2 | 3) || self.sel_range().is_some();
                        if enabled {
                            self.do_action(i);
                        }
                        self.ctx_open = false;
                        self.focus_time = now;
                        return EventResult::REDRAW;
                    }
                    return EventResult::NONE;
                }
                _ => return EventResult::NONE,
            }
        }

        let mut redraw = false;
        match ev {
            InputEvent::ContextMenu(p) => {
                if self.rect.contains(p) {
                    self.focused = true;
                    self.focus_time = now;
                    self.ctx_open = true;
                    self.ctx_pos = p;
                    self.ctx_hover = None;
                    redraw = true;
                }
            }
            InputEvent::PointerMove(p) => {
                let h = self.rect.contains(p);
                if h != self.hovered {
                    self.hovered = h;
                    redraw = true;
                }
            }
            InputEvent::PointerLeave => {
                if self.hovered {
                    self.hovered = false;
                    redraw = true;
                }
            }
            InputEvent::PointerDown(p) => {
                let inside = self.rect.contains(p);
                if inside != self.focused {
                    self.focused = inside;
                    redraw = true;
                }
                if inside {
                    self.caret = self.char_len();
                    self.sel_anchor = None;
                    self.focus_time = now;
                    redraw = true;
                }
            }
            InputEvent::Char(c) => {
                if self.focused && !c.is_control() {
                    self.insert_str(&c.to_string());
                    self.focus_time = now;
                    redraw = true;
                }
            }
            InputEvent::KeyDown(vk) if self.focused => {
                match vk {
                    0x08 => {
                        if !self.delete_selection() && self.caret > 0 {
                            let start = byte_index(&self.text, self.caret - 1);
                            let end = byte_index(&self.text, self.caret);
                            self.text.replace_range(start..end, "");
                            self.caret -= 1;
                        }
                        redraw = true;
                    }
                    0x2E => {
                        if !self.delete_selection() {
                            let n = self.char_len();
                            if self.caret < n {
                                let start = byte_index(&self.text, self.caret);
                                let end = byte_index(&self.text, self.caret + 1);
                                self.text.replace_range(start..end, "");
                            }
                        }
                        redraw = true;
                    }
                    0x25 => { self.sel_anchor = None; if self.caret > 0 { self.caret -= 1; } redraw = true; }
                    0x27 => { self.sel_anchor = None; if self.caret < self.char_len() { self.caret += 1; } redraw = true; }
                    0x24 => { self.sel_anchor = None; self.caret = 0; redraw = true; }
                    0x23 => { self.sel_anchor = None; self.caret = self.char_len(); redraw = true; }
                    0x41 => { /* Ctrl+A 由 KeyDown 无修饰位，简化：用菜单全选 */ }
                    _ => {}
                }
                self.focus_time = now;
            }
            _ => {}
        }
        EventResult { redraw, animating: false }
    }

    fn is_animating(&self, _now: f64) -> bool {
        self.focused && self.sel_range().is_none()
    }

    fn wants_keyboard(&self) -> bool {
        self.focused
    }

    fn wants_modal(&self) -> bool {
        self.ctx_open
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::ComboBox
    }
    fn accessible_name(&self) -> String {
        self.text.clone()
    }
}

/// 字符索引 → 字节索引（UTF-8 安全编辑）。
fn byte_index(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map(|(b, _)| b).unwrap_or(s.len())
}
