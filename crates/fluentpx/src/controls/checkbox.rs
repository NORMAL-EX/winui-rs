//! CheckBox（复选框，含三态 + 勾「描线」动画）。
//!
//! 真值来源：`controls/dev/CommonStyles/CheckBox_themeresources.xaml`
//! * 方框 20×20，圆角 4（ControlCornerRadius），边框 1
//! * Unchecked：描边 `ControlStrongStrokeColorDefault`；Checked：填充 `AccentFillColorDefault`
//!   (hover Secondary / pressed Tertiary)，勾 `TextOnAccentFillColorPrimary`，glyph 12
//! * 勾用 AnimatedAccept「描线」动画：此处用折线按长度逐段揭示（~200ms FastOutSlowIn）复刻
//! * Indeterminate：实心中点短横（accent 底 + OnAccent 横）

use crate::anim::ease_out;
use crate::typography::TextStyle;
use crate::widget::*;

const BOX: f32 = 20.0;
const CORNER: f32 = 4.0;
const BORDER: f32 = 1.0;
const LABEL_GAP: f32 = 8.0;
const DRAW_DUR: f64 = 0.2;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Unchecked,
    Checked,
    Indeterminate,
}

pub struct CheckBox {
    pub state: CheckState,
    pub label: String,
    pub interaction: Interaction,
    rect: Rect,
    anim_start: f64, // 勾描线起点
}

impl CheckBox {
    pub fn new(label: impl Into<String>, state: CheckState) -> CheckBox {
        CheckBox { state, label: label.into(), interaction: Interaction::default(), rect: Rect::default(), anim_start: -1.0 }
    }

    fn box_rect(&self) -> Rect {
        Rect { x: self.rect.x, y: self.rect.center_y() - BOX / 2.0, w: BOX, h: BOX }
    }

    fn toggle(&mut self, now: f64) {
        // 二态切换（Indeterminate 视作已选，点击后变 Unchecked）。
        self.state = match self.state {
            CheckState::Unchecked => CheckState::Checked,
            _ => CheckState::Unchecked,
        };
        if self.state == CheckState::Checked {
            self.anim_start = now;
        }
    }
}

impl Widget for CheckBox {
    fn measure(&mut self, _available: Size) -> Size {
        let w = if self.label.is_empty() { BOX } else { BOX + LABEL_GAP + self.label.chars().count() as f32 * 8.0 };
        Size { w, h: 32.0 }
    }

    fn arrange(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn hit_test(&self, p: Point) -> bool {
        self.rect.contains(p)
    }

    fn paint(&mut self, ctx: &mut PaintCtx) {
        let t = ctx.tokens;
        let b = self.box_rect();
        let vs = self.interaction.visual_state();
        let checked = self.state != CheckState::Unchecked;

        if checked {
            // 实心 accent 方框（按交互态取 default/secondary/tertiary）。
            let fill = if !self.interaction.enabled {
                t.accent_fill_disabled
            } else {
                match vs {
                    VisualState::PointerOver => t.accent_fill_secondary(),
                    VisualState::Pressed => t.accent_fill_tertiary(),
                    _ => t.accent_fill_default(),
                }
            };
            ctx.painter.fill_rounded_rect(b, CORNER, fill);
        } else {
            // 空框：弱填充 + 强描边。
            let fill = match vs {
                VisualState::PointerOver => t.control_alt_fill_tertiary,
                VisualState::Pressed => t.control_alt_fill_quarternary,
                _ => t.control_alt_fill_secondary,
            };
            ctx.painter.fill_rounded_rect(b.inset(BORDER), (CORNER - BORDER).max(0.0), fill);
            let stroke = if self.interaction.enabled { t.strong_stroke_default } else { t.strong_stroke_disabled };
            ctx.painter.stroke_inner(b, CORNER, stroke, BORDER);
        }

        let glyph = if self.interaction.enabled { t.text_on_accent_primary } else { t.text_on_accent_disabled };
        match self.state {
            CheckState::Checked => {
                // 勾折线（box 单位坐标），按描线进度逐段揭示。
                let p0 = (b.x + BOX * 0.27, b.y + BOX * 0.54);
                let p1 = (b.x + BOX * 0.43, b.y + BOX * 0.68);
                let p2 = (b.x + BOX * 0.73, b.y + BOX * 0.33);
                let prog = if self.anim_start < 0.0 {
                    1.0
                } else {
                    ease_out(((ctx.now - self.anim_start) / DRAW_DUR).clamp(0.0, 1.0) as f32)
                };
                let pts = partial_polyline(&[p0, p1, p2], prog);
                ctx.painter.stroke_polyline(&pts, glyph, 1.6);
            }
            CheckState::Indeterminate => {
                let dash = Rect { x: b.x + BOX * 0.28, y: b.center_y() - 1.0, w: BOX * 0.44, h: 2.0 };
                ctx.painter.fill_rounded_rect(dash, 1.0, glyph);
            }
            CheckState::Unchecked => {}
        }

        // 标签
        if !self.label.is_empty() {
            let fg = if self.interaction.enabled { t.text_primary } else { t.text_disabled };
            let lr = Rect { x: b.right() + LABEL_GAP, y: self.rect.y, w: self.rect.w - BOX - LABEL_GAP, h: self.rect.h };
            let _ = ctx.painter.draw_text_leading(&self.label, TextStyle::BODY, lr, fg);
        }
    }

    fn on_event(&mut self, ev: InputEvent, now: f64) -> EventResult {
        if !self.interaction.enabled {
            return EventResult::NONE;
        }
        let before = self.interaction;
        let mut toggled = false;
        match ev {
            InputEvent::PointerMove(p) => self.interaction.hovered = self.rect.contains(p),
            InputEvent::PointerLeave => {
                self.interaction.hovered = false;
                self.interaction.pressed = false;
            }
            InputEvent::PointerDown(p) => {
                if self.rect.contains(p) {
                    self.interaction.pressed = true;
                    self.interaction.focused = true;
                }
            }
            InputEvent::PointerUp(p) => {
                if self.interaction.pressed && self.rect.contains(p) {
                    self.toggle(now);
                    toggled = true;
                }
                self.interaction.pressed = false;
            }
            InputEvent::KeyDown(vk) => {
                if vk == 0x20 {
                    self.toggle(now);
                    toggled = true;
                }
            }
            _ => {}
        }
        let changed = toggled || before.visual_state() != self.interaction.visual_state();
        EventResult { redraw: changed, animating: changed || toggled }
    }

    fn is_animating(&self, now: f64) -> bool {
        self.anim_start >= 0.0 && (now - self.anim_start) < DRAW_DUR
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::CheckBox
    }
    fn accessible_name(&self) -> String {
        self.label.clone()
    }
}

/// 取折线前 `frac` 长度比例的子折线（用于「描线」揭示）。
fn partial_polyline(pts: &[(f32, f32)], frac: f32) -> Vec<(f32, f32)> {
    if frac >= 1.0 || pts.len() < 2 {
        return pts.to_vec();
    }
    if frac <= 0.0 {
        return vec![pts[0]];
    }
    let mut seg = Vec::with_capacity(pts.len());
    let mut total = 0.0f32;
    for w in pts.windows(2) {
        total += dist(w[0], w[1]);
    }
    let target = total * frac;
    let mut acc = 0.0f32;
    seg.push(pts[0]);
    for w in pts.windows(2) {
        let d = dist(w[0], w[1]);
        if acc + d >= target {
            let r = (target - acc) / d.max(0.0001);
            seg.push((w[0].0 + (w[1].0 - w[0].0) * r, w[0].1 + (w[1].1 - w[0].1) * r));
            break;
        }
        seg.push(w[1]);
        acc += d;
    }
    seg
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}
