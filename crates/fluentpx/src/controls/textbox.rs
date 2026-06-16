//! TextBox（编辑框，单行）。
//!
//! 真值参考：`controls/dev/CommonStyles/TextBox_themeresources.xaml`
//! * MinHeight 32，圆角 4，背景 `ControlFillColorDefault`（聚焦 `ControlFillColorInputActive`）
//! * 边框 `ControlElevationBorderBrush`；聚焦时底部 2px `SystemAccentColor` 焦点下划线
//! * 前景 `TextFillColorPrimary`，占位符 `TextFillColorTertiary`
//!
//! 限制：仅处理已翻译字符（WM_CHAR）与基本编辑键，**中文 IME 需对接 TSF**，列为后续。
//! 光标闪烁周期 ~530ms。

use crate::typography::TextStyle;
use crate::widget::*;

const BOX_H: f32 = 32.0;
const CORNER: f32 = 4.0;
const BORDER: f32 = 1.0;
const PAD_L: f32 = 11.0;
const FOCUS_LINE: f32 = 2.0;
const BLINK_MS: f64 = 530.0;

pub struct TextBox {
    pub text: String,
    pub placeholder: String,
    caret: usize, // 字符索引
    focused: bool,
    hovered: bool,
    rect: Rect,
    pub enabled: bool,
    focus_time: f64,
}

impl TextBox {
    pub fn new(placeholder: impl Into<String>) -> TextBox {
        TextBox {
            text: String::new(),
            placeholder: placeholder.into(),
            caret: 0,
            focused: false,
            hovered: false,
            rect: Rect::default(),
            enabled: true,
            focus_time: 0.0,
        }
    }

    fn caret_visible(&self, now: f64) -> bool {
        if !self.focused {
            return false;
        }
        let elapsed_ms = (now - self.focus_time) * 1000.0;
        ((elapsed_ms / BLINK_MS) as i64) % 2 == 0
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
        self.rect.contains(p)
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

        // 文字 / 占位符
        let inner = Rect { x: r.x + PAD_L, y: r.y, w: r.w - PAD_L * 2.0, h: r.h };
        if self.text.is_empty() && !self.focused {
            let _ = ctx.painter.draw_text_leading(&self.placeholder, TextStyle::BODY, inner, t.text_tertiary);
        } else {
            let fg = if self.enabled { t.text_primary } else { t.text_disabled };
            let _ = ctx.painter.draw_text_leading(&self.text, TextStyle::BODY, inner, fg);
        }

        // 光标（量出 caret 前文本宽度定位）
        if self.caret_visible(ctx.now) {
            let prefix: String = self.text.chars().take(self.caret).collect();
            let w = ctx
                .painter
                .measure_text(&prefix, TextStyle::BODY)
                .map(|s| s.w)
                .unwrap_or(0.0);
            let cx = inner.x + w;
            ctx.painter.draw_line(cx, r.y + 7.0, cx, r.bottom() - 7.0, t.text_primary, 1.0);
        }

        // 聚焦底部 2px accent 下划线
        if self.focused {
            let line = Rect { x: r.x, y: r.bottom() - FOCUS_LINE, w: r.w, h: FOCUS_LINE };
            ctx.painter.fill_rounded_rect(line, FOCUS_LINE / 2.0, t.accent_fill_default());
        }
    }

    fn on_event(&mut self, ev: InputEvent, now: f64) -> EventResult {
        if !self.enabled {
            return EventResult::NONE;
        }
        let mut redraw = false;
        match ev {
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
                    self.focus_time = now;
                    redraw = true;
                }
                if inside {
                    self.caret = self.text.chars().count();
                    self.focus_time = now;
                    redraw = true;
                }
            }
            InputEvent::Char(c) => {
                if self.focused && !c.is_control() {
                    let idx = byte_index(&self.text, self.caret);
                    self.text.insert(idx, c);
                    self.caret += 1;
                    self.focus_time = now;
                    redraw = true;
                }
            }
            InputEvent::KeyDown(vk) if self.focused => {
                match vk {
                    0x08 => {
                        // Backspace
                        if self.caret > 0 {
                            let start = byte_index(&self.text, self.caret - 1);
                            let end = byte_index(&self.text, self.caret);
                            self.text.replace_range(start..end, "");
                            self.caret -= 1;
                            redraw = true;
                        }
                    }
                    0x2E => {
                        // Delete
                        let n = self.text.chars().count();
                        if self.caret < n {
                            let start = byte_index(&self.text, self.caret);
                            let end = byte_index(&self.text, self.caret + 1);
                            self.text.replace_range(start..end, "");
                            redraw = true;
                        }
                    }
                    0x25 => { if self.caret > 0 { self.caret -= 1; redraw = true; } } // Left
                    0x27 => { if self.caret < self.text.chars().count() { self.caret += 1; redraw = true; } } // Right
                    0x24 => { self.caret = 0; redraw = true; } // Home
                    0x23 => { self.caret = self.text.chars().count(); redraw = true; } // End
                    _ => {}
                }
                self.focus_time = now;
            }
            _ => {}
        }
        EventResult { redraw, animating: false }
    }

    fn is_animating(&self, _now: f64) -> bool {
        self.focused // 光标闪烁需持续刷新
    }

    fn wants_keyboard(&self) -> bool {
        self.focused
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
