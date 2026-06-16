//! ComboBox：闭合态 + 展开弹出列表。
//!
//! 真值来源：`controls/dev/ComboBox/ComboBox_themeresources.xaml`
//! * MinHeight 32，Padding `12,5,0,7`，圆角 4
//! * 背景 `ControlFillColorDefault`(rest)/`Secondary`(hover)/`Tertiary`(pressed)
//! * 边框 `ControlElevationBorderBrush`（渐变），前景 `TextFillColorPrimary`
//! * 右侧 ChevronDown 字形（Segoe Fluent Icons `\u{E70D}`）12px，右距 14
//! * 下拉 `OverlayCornerRadius 8`，背景 `AcrylicInAppFillColorDefault`（acrylic，
//!   此处用 `SolidBackgroundFillColorTertiary` 纯色近似，**待办：实时模糊**），
//!   边框 1px，内容上下 margin 4，item 高 32
//!
//! 限制：本实现为**不可编辑** ComboBox。可编辑 + 中文输入需对接 TSF，列为后续。

use crate::anim::ease_out;
use crate::color::Color;
use crate::gfx::Icon;
use crate::typography::TextStyle;
use crate::widget::*;

const BOX_H: f32 = 32.0;
const CORNER: f32 = 4.0;
const BORDER: f32 = 1.0;
const PAD_L: f32 = 12.0;
const CHEVRON_SIZE: f32 = 12.0;
const CHEVRON_RIGHT: f32 = 14.0;
const POPUP_CORNER: f32 = 8.0;
const ITEM_H: f32 = 32.0;
const POPUP_VPAD: f32 = 4.0;
const IND_W: f32 = 3.0;
const IND_H: f32 = 16.0;
const OPEN_DUR: f64 = 0.15;

pub struct ComboBox {
    pub items: Vec<String>,
    pub selected: usize,
    pub open: bool,
    hovered_box: bool,
    pressed_box: bool,
    hovered_item: Option<usize>,
    rect: Rect,
    pub enabled: bool,
    open_start: f64,
}

impl ComboBox {
    pub fn new(items: Vec<String>, selected: usize) -> ComboBox {
        ComboBox {
            items,
            selected,
            open: false,
            hovered_box: false,
            pressed_box: false,
            hovered_item: None,
            rect: Rect::default(),
            enabled: true,
            open_start: 0.0,
        }
    }

    fn popup_rect(&self) -> Rect {
        let h = self.items.len() as f32 * ITEM_H + POPUP_VPAD * 2.0;
        Rect { x: self.rect.x, y: self.rect.bottom() + 2.0, w: self.rect.w, h }
    }

    fn popup_item_rect(&self, i: usize) -> Rect {
        let p = self.popup_rect();
        Rect { x: p.x, y: p.y + POPUP_VPAD + i as f32 * ITEM_H, w: p.w, h: ITEM_H }
    }

    fn item_at(&self, pt: Point) -> Option<usize> {
        if !self.popup_rect().contains(pt) {
            return None;
        }
        (0..self.items.len()).find(|&i| self.popup_item_rect(i).contains(pt))
    }
}

impl Widget for ComboBox {
    fn measure(&mut self, available: Size) -> Size {
        Size { w: available.w.clamp(200.0, 300.0), h: BOX_H }
    }

    fn arrange(&mut self, rect: Rect) {
        self.rect = Rect { x: rect.x, y: rect.y, w: rect.w, h: BOX_H };
    }

    fn hit_test(&self, p: Point) -> bool {
        self.rect.contains(p) || (self.open && self.popup_rect().contains(p))
    }

    fn paint(&mut self, ctx: &mut PaintCtx) {
        let t = ctx.tokens;
        let r = self.rect;
        let bg = if !self.enabled {
            t.control_fill_disabled
        } else if self.pressed_box || self.open {
            t.control_fill_tertiary
        } else if self.hovered_box {
            t.control_fill_secondary
        } else {
            t.control_fill_default
        };
        ctx.painter.fill_rounded_rect(r.inset(BORDER), (CORNER - BORDER).max(0.0), bg);
        let _ = ctx.painter.stroke_inner_gradient(r, CORNER, &t.control_elevation_border(), BORDER);

        // 选中项文字
        let fg = if self.enabled { t.text_primary } else { t.text_disabled };
        let text_rect = Rect { x: r.x + PAD_L, y: r.y, w: r.w - PAD_L - CHEVRON_RIGHT - CHEVRON_SIZE, h: r.h };
        if let Some(s) = self.items.get(self.selected) {
            let _ = ctx.painter.draw_text_leading(s, TextStyle::BODY, text_rect, fg);
        }

        // 右侧 chevron
        let gly = Rect {
            x: r.right() - CHEVRON_RIGHT - CHEVRON_SIZE,
            y: r.center_y() - CHEVRON_SIZE / 2.0,
            w: CHEVRON_SIZE,
            h: CHEVRON_SIZE,
        };
        ctx.painter.draw_glyph(Icon::ChevronDown, gly, if self.enabled { t.text_secondary } else { t.text_disabled });
    }

    fn paint_overlay(&mut self, ctx: &mut PaintCtx) {
        if !self.open {
            return;
        }
        let t = ctx.tokens;
        // 开合动画：透明度 + 纵向位移（与 Menu 一致的弹出观感）。
        let prog = ease_out(((ctx.now - self.open_start) / OPEN_DUR).clamp(0.0, 1.0) as f32);
        let dy = (1.0 - prog) * -8.0;
        let alpha = prog;
        let base = self.popup_rect();
        let p = Rect { y: base.y + dy, ..base };

        // 阴影近似
        ctx.painter.fill_rounded_rect(Rect { y: p.y + 2.0, ..p }, POPUP_CORNER, Color::hex("#30000000").with_opacity(alpha));
        // 背景（acrylic 近似）+ 边框
        ctx.painter.fill_rounded_rect(p, POPUP_CORNER, t.solid_bg_tertiary.with_opacity(alpha));
        ctx.painter.stroke_inner(p, POPUP_CORNER, t.surface_stroke_flyout.with_opacity(alpha), 1.0);

        for i in 0..self.items.len() {
            let mut ir = self.popup_item_rect(i);
            ir.y += dy;
            let selected = i == self.selected;
            let hovered = self.hovered_item == Some(i);
            let bg = if selected && hovered {
                t.subtle_fill_tertiary
            } else if selected || hovered {
                t.subtle_fill_secondary
            } else {
                Color::TRANSPARENT
            };
            if bg.a != 0 {
                ctx.painter.fill_rounded_rect(Rect { x: ir.x + 4.0, y: ir.y + 1.0, w: ir.w - 8.0, h: ir.h - 2.0 }, 4.0, bg.with_opacity(alpha));
            }
            if selected {
                let ind = Rect { x: ir.x + 4.0, y: ir.center_y() - IND_H / 2.0, w: IND_W, h: IND_H };
                ctx.painter.fill_rounded_rect(ind, 1.5, t.accent_fill_default().with_opacity(alpha));
            }
            let text_rect = Rect { x: ir.x + PAD_L, y: ir.y, w: ir.w - PAD_L - 8.0, h: ir.h };
            let _ = ctx.painter.draw_text_leading(&self.items[i], TextStyle::BODY, text_rect, t.text_primary.with_opacity(alpha));
        }
    }

    fn on_event(&mut self, ev: InputEvent, now: f64) -> EventResult {
        if !self.enabled {
            return EventResult::NONE;
        }
        let mut redraw = false;
        match ev {
            InputEvent::PointerMove(pt) => {
                let hb = self.rect.contains(pt);
                if hb != self.hovered_box {
                    self.hovered_box = hb;
                    redraw = true;
                }
                if self.open {
                    let hi = self.item_at(pt);
                    if hi != self.hovered_item {
                        self.hovered_item = hi;
                        redraw = true;
                    }
                }
            }
            InputEvent::PointerLeave => {
                self.hovered_box = false;
                self.hovered_item = None;
                redraw = true;
            }
            InputEvent::PointerDown(pt) => {
                if self.rect.contains(pt) {
                    self.pressed_box = true;
                    redraw = true;
                } else if self.open && !self.popup_rect().contains(pt) {
                    // 点击外部关闭
                    self.open = false;
                    redraw = true;
                }
            }
            InputEvent::PointerUp(pt) => {
                if self.pressed_box && self.rect.contains(pt) {
                    self.open = !self.open;
                    if self.open {
                        self.open_start = now;
                    }
                    redraw = true;
                } else if self.open {
                    if let Some(i) = self.item_at(pt) {
                        self.selected = i;
                        self.open = false;
                        redraw = true;
                    }
                }
                self.pressed_box = false;
            }
            _ => {}
        }
        EventResult { redraw, animating: self.open }
    }

    fn is_animating(&self, now: f64) -> bool {
        self.open && (now - self.open_start) < OPEN_DUR
    }

    fn wants_modal(&self) -> bool {
        self.open
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::ComboBox
    }
}
