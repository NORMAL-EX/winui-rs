//! ListView / ListBox：item 选中/悬停态 + 左侧选中指示条。
//!
//! 真值来源：`controls/dev/CommonStyles/ListViewItem_themeresources.xaml`
//! * Item MinHeight 40，圆角 4（ControlCornerRadius）
//! * 背景：rest 透明；PointerOver `SubtleFillColorSecondary`；Selected `SubtleFillColorSecondary`；
//!   SelectedPointerOver `SubtleFillColorTertiary`
//! * 前景 `TextFillColorPrimary`
//! * 选中指示条：`AccentFillColorDefault`，CornerRadius 1.5，宽 3、高 16，左侧垂直居中

use crate::color::Color;
use crate::widget::*;

const ITEM_H: f32 = 40.0;
const ITEM_CORNER: f32 = 4.0;
const INDICATOR_W: f32 = 3.0;
const INDICATOR_H: f32 = 16.0;
const INDICATOR_R: f32 = 1.5;
const CONTENT_LEFT: f32 = 14.0;

pub struct ListView {
    pub items: Vec<String>,
    pub selected: Option<usize>,
    hovered: Option<usize>,
    pressed: Option<usize>,
    rect: Rect,
    pub enabled: bool,
}

impl ListView {
    pub fn new(items: Vec<String>, selected: Option<usize>) -> ListView {
        ListView { items, selected, hovered: None, pressed: None, rect: Rect::default(), enabled: true }
    }

    fn item_rect(&self, i: usize) -> Rect {
        Rect { x: self.rect.x, y: self.rect.y + i as f32 * ITEM_H, w: self.rect.w, h: ITEM_H }
    }

    fn index_at(&self, p: Point) -> Option<usize> {
        if !self.rect.contains(p) {
            return None;
        }
        let i = ((p.y - self.rect.y) / ITEM_H).floor() as i32;
        if i >= 0 && (i as usize) < self.items.len() {
            Some(i as usize)
        } else {
            None
        }
    }
}

impl Widget for ListView {
    fn measure(&mut self, available: Size) -> Size {
        Size { w: available.w.max(240.0), h: ITEM_H * self.items.len() as f32 }
    }

    fn arrange(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn hit_test(&self, p: Point) -> bool {
        self.rect.contains(p)
    }

    fn paint(&mut self, ctx: &mut PaintCtx) {
        let t = ctx.tokens;
        for i in 0..self.items.len() {
            let r = self.item_rect(i);
            let selected = self.selected == Some(i);
            let hovered = self.hovered == Some(i);
            let pressed = self.pressed == Some(i);

            // 背景态
            let bg = match (selected, hovered, pressed) {
                (true, _, true) => t.subtle_fill_secondary,
                (true, true, _) => t.subtle_fill_tertiary,
                (true, false, _) => t.subtle_fill_secondary,
                (false, _, true) => t.subtle_fill_tertiary,
                (false, true, _) => t.subtle_fill_secondary,
                _ => Color::TRANSPARENT,
            };
            if bg.a != 0 {
                ctx.painter.fill_rounded_rect(r.inset(2.0), ITEM_CORNER, bg);
            }

            // 选中指示条（左侧 accent 药丸）
            if selected {
                let ind = Rect {
                    x: r.x + 1.0,
                    y: r.center_y() - INDICATOR_H / 2.0,
                    w: INDICATOR_W,
                    h: INDICATOR_H,
                };
                ctx.painter.fill_rounded_rect(ind, INDICATOR_R, t.accent_fill_default());
            }

            // 文字
            let fg = if self.enabled { t.text_primary } else { t.text_disabled };
            let text_rect = Rect { x: r.x + CONTENT_LEFT, y: r.y, w: r.w - CONTENT_LEFT - 8.0, h: r.h };
            let _ = ctx.painter.draw_text_leading(&self.items[i], crate::typography::TextStyle::BODY, text_rect, fg);
        }
    }

    fn on_event(&mut self, ev: InputEvent, _now: f64) -> EventResult {
        if !self.enabled {
            return EventResult::NONE;
        }
        let mut redraw = false;
        match ev {
            InputEvent::PointerMove(p) => {
                let h = self.index_at(p);
                if h != self.hovered {
                    self.hovered = h;
                    redraw = true;
                }
            }
            InputEvent::PointerLeave => {
                if self.hovered.is_some() || self.pressed.is_some() {
                    self.hovered = None;
                    self.pressed = None;
                    redraw = true;
                }
            }
            InputEvent::PointerDown(p) => {
                self.pressed = self.index_at(p);
                redraw = self.pressed.is_some();
            }
            InputEvent::PointerUp(p) => {
                if let Some(i) = self.index_at(p) {
                    if self.pressed == Some(i) {
                        self.selected = Some(i);
                        redraw = true;
                    }
                }
                self.pressed = None;
            }
            _ => {}
        }
        EventResult { redraw, animating: false }
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::List
    }
}
