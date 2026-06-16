//! InfoBar（通知条）：Informational / Success / Warning / Error 四态。
//!
//! 真值来源：`controls/dev/InfoBar/*` + Common_themeresources（SystemFillColor* 系列）。
//! * 背景：Informational=`SystemFillColorAttentionBackground`，Success=`...SuccessBackground`，
//!   Warning=`...CautionBackground`，Error=`...CriticalBackground`
//! * 图标色：Success=`SystemFillColorSuccess`，Warning=`...Caution`，Error=`...Critical`，
//!   Informational=强调色
//! * 圆角 4，边框 1px（CardStroke/Divider），标题 BodyStrong + 正文 Body
//! 图标用 Segoe MDL2 Assets 字形；关闭按钮用两条对角线绘制（避免字形缺失）。

use crate::color::Color;
use crate::typography::TextStyle;
use crate::widget::*;

const H: f32 = 60.0;
const CORNER: f32 = 4.0;
const ICON_X: f32 = 16.0;
const TEXT_X: f32 = 48.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Informational,
    Success,
    Warning,
    Error,
}

pub struct InfoBar {
    pub severity: Severity,
    pub title: String,
    pub message: String,
    pub closed: bool,
    hover_close: bool,
    rect: Rect,
}

impl InfoBar {
    pub fn new(severity: Severity, title: impl Into<String>, message: impl Into<String>) -> InfoBar {
        InfoBar { severity, title: title.into(), message: message.into(), closed: false, hover_close: false, rect: Rect::default() }
    }

    fn glyph(&self) -> char {
        match self.severity {
            Severity::Informational => '\u{E946}', // Info
            Severity::Success => '\u{E930}',       // Completed
            Severity::Warning => '\u{E7BA}',       // Warning
            Severity::Error => '\u{EA39}',         // ErrorBadge
        }
    }

    fn bg(&self, t: &crate::tokens::Tokens) -> Color {
        match self.severity {
            Severity::Informational => t.system_attention_bg,
            Severity::Success => t.system_success_bg,
            Severity::Warning => t.system_caution_bg,
            Severity::Error => t.system_critical_bg,
        }
    }

    fn icon_color(&self, t: &crate::tokens::Tokens) -> Color {
        match self.severity {
            Severity::Informational => t.accent_fill_default(),
            Severity::Success => t.system_success,
            Severity::Warning => t.system_caution,
            Severity::Error => t.system_critical,
        }
    }

    fn close_rect(&self) -> Rect {
        Rect { x: self.rect.right() - 40.0, y: self.rect.center_y() - 16.0, w: 32.0, h: 32.0 }
    }
}

impl Widget for InfoBar {
    fn measure(&mut self, available: Size) -> Size {
        if self.closed {
            return Size { w: 0.0, h: 0.0 };
        }
        Size { w: available.w.max(360.0), h: H }
    }

    fn arrange(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn hit_test(&self, p: Point) -> bool {
        !self.closed && self.rect.contains(p)
    }

    fn paint(&mut self, ctx: &mut PaintCtx) {
        if self.closed {
            return;
        }
        let t = ctx.tokens;
        let r = self.rect;
        ctx.painter.fill_rounded_rect(r, CORNER, self.bg(t));
        ctx.painter.stroke_inner(r, CORNER, t.card_bg_default, 1.0);

        // 图标
        let icon = Rect { x: r.x + ICON_X, y: r.center_y() - 10.0, w: 20.0, h: 20.0 };
        let _ = ctx.painter.draw_icon(self.glyph(), 16.0, icon, self.icon_color(t));

        // 标题 + 正文
        let title_rect = Rect { x: r.x + TEXT_X, y: r.y + 10.0, w: r.w - TEXT_X - 48.0, h: 20.0 };
        let _ = ctx.painter.draw_text_leading(&self.title, TextStyle::BODY_STRONG, title_rect, t.text_primary);
        let msg_rect = Rect { x: r.x + TEXT_X, y: r.y + 30.0, w: r.w - TEXT_X - 48.0, h: 20.0 };
        let _ = ctx.painter.draw_text_leading(&self.message, TextStyle::BODY, msg_rect, t.text_secondary);

        // 关闭按钮（hover 底 + 两条对角线画 ✕）
        let cr = self.close_rect();
        if self.hover_close {
            ctx.painter.fill_rounded_rect(cr, 4.0, t.subtle_fill_secondary);
        }
        let cc = t.text_secondary;
        let cx = cr.center_x();
        let cy = cr.center_y();
        ctx.painter.draw_line(cx - 5.0, cy - 5.0, cx + 5.0, cy + 5.0, cc, 1.0);
        ctx.painter.draw_line(cx + 5.0, cy - 5.0, cx - 5.0, cy + 5.0, cc, 1.0);
    }

    fn on_event(&mut self, ev: InputEvent, _now: f64) -> EventResult {
        if self.closed {
            return EventResult::NONE;
        }
        let mut redraw = false;
        match ev {
            InputEvent::PointerMove(p) => {
                let h = self.close_rect().contains(p);
                if h != self.hover_close {
                    self.hover_close = h;
                    redraw = true;
                }
            }
            InputEvent::PointerLeave => {
                if self.hover_close {
                    self.hover_close = false;
                    redraw = true;
                }
            }
            InputEvent::PointerUp(p) => {
                if self.close_rect().contains(p) {
                    self.closed = true;
                    redraw = true;
                }
            }
            _ => {}
        }
        EventResult { redraw, animating: false }
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::ToolTip
    }
}
