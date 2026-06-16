//! ToggleSwitch（开关），含 knob 位移动画 + 缓动。
//!
//! 真值来源：`controls/dev/CommonStyles/ToggleSwitch_themeresources.xaml`
//! * 轨道 OuterBorder：Width 40、Height 20、Radius 10、StrokeThickness 1（off）
//! * Knob：12×12（CornerRadius 7 → 圆），KnobTranslateTransform.X：off=0 → on=20
//! * Off 轨道填充 `ControlAltFillColorSecondary`（hover Tertiary / pressed Quarternary），
//!   描边 `ControlStrongStrokeColorDefault`，knob `TextFillColorSecondary`
//! * On 轨道填充 `AccentFillColorDefault`（无描边），knob `TextOnAccentFillColorPrimary`(#000000)
//!
//! 轨道 off→on 用同一进度 p 做不透明度交叉淡入；knob 位置与颜色随 p 缓动（FastOutSlowIn）。
//! 注：knob 按下时的「拉长」形变（press morph）列为后续细化，先实现位移+配色交叉淡入。

use crate::anim::{ease_out, lerp, lerp_color};
use crate::widget::*;

const TRACK_W: f32 = 40.0;
const TRACK_H: f32 = 20.0;
const TRACK_R: f32 = 10.0;
const TRACK_STROKE: f32 = 1.0;
const KNOB_D: f32 = 12.0;
const KNOB_OFF_CX: f32 = 10.0; // 轨道局部坐标
const KNOB_ON_CX: f32 = 30.0; // = 10 + TranslateX(20)
const TOGGLE_DUR: f64 = 0.15;

pub struct ToggleSwitch {
    pub on: bool,
    pub on_text: String,
    pub off_text: String,
    pub interaction: Interaction,
    rect: Rect,
    // 动画进度：0=off, 1=on。
    anim_from: f32,
    anim_to: f32,
    anim_start: f64,
}

impl ToggleSwitch {
    pub fn new(on: bool) -> ToggleSwitch {
        ToggleSwitch {
            on,
            on_text: "On".into(),
            off_text: "Off".into(),
            interaction: Interaction::default(),
            rect: Rect::default(),
            anim_from: if on { 1.0 } else { 0.0 },
            anim_to: if on { 1.0 } else { 0.0 },
            anim_start: 0.0,
        }
    }

    fn progress(&self, now: f64) -> f32 {
        if (now - self.anim_start) >= TOGGLE_DUR {
            return self.anim_to;
        }
        let t = ((now - self.anim_start) / TOGGLE_DUR).clamp(0.0, 1.0) as f32;
        lerp(self.anim_from, self.anim_to, ease_out(t))
    }

    fn toggle(&mut self, now: f64) {
        self.on = !self.on;
        self.anim_from = self.anim_to; // 从当前目标继续（瞬时点击间隔足够）
        // 若动画进行中，从当前可见进度起步，保证连续
        let cur = self.progress(now);
        self.anim_from = cur;
        self.anim_to = if self.on { 1.0 } else { 0.0 };
        self.anim_start = now;
    }

    /// 轨道矩形（在控件 rect 内垂直居中、左对齐）。
    fn track(&self) -> Rect {
        Rect {
            x: self.rect.x,
            y: self.rect.center_y() - TRACK_H / 2.0,
            w: TRACK_W,
            h: TRACK_H,
        }
    }
}

impl Widget for ToggleSwitch {
    fn measure(&mut self, _available: Size) -> Size {
        Size { w: TRACK_W, h: TRACK_H }
    }

    fn arrange(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn hit_test(&self, p: Point) -> bool {
        self.rect.contains(p)
    }

    fn paint(&mut self, ctx: &mut PaintCtx) {
        let t = ctx.tokens;
        let p = self.progress(ctx.now);
        let track = self.track();
        let vs = self.interaction.visual_state();

        // —— off 轨道（始终绘制）——
        let off_fill = if !self.interaction.enabled {
            t.control_alt_fill_disabled
        } else {
            match vs {
                VisualState::PointerOver => t.control_alt_fill_tertiary,
                VisualState::Pressed => t.control_alt_fill_quarternary,
                _ => t.control_alt_fill_secondary,
            }
        };
        ctx.painter.fill_rounded_rect(track, TRACK_R, off_fill);
        let off_stroke = if self.interaction.enabled { t.strong_stroke_default } else { t.strong_stroke_disabled };
        ctx.painter.stroke_inner(track, TRACK_R, off_stroke, TRACK_STROKE);

        // —— on 轨道（accent）以不透明度 p 交叉淡入 ——
        if p > 0.0 {
            let on_fill = if !self.interaction.enabled {
                t.accent_fill_disabled
            } else {
                match vs {
                    VisualState::PointerOver => t.accent_fill_secondary(),
                    VisualState::Pressed => t.accent_fill_tertiary(),
                    _ => t.accent_fill_default(),
                }
            };
            ctx.painter.fill_rounded_rect(track, TRACK_R, on_fill.with_opacity(p));
        }

        // —— knob：位置 + 颜色随 p 缓动 ——
        let cx = track.x + lerp(KNOB_OFF_CX, KNOB_ON_CX, p);
        let cy = track.center_y();
        let off_knob = if self.interaction.enabled { t.text_secondary } else { t.text_disabled };
        let on_knob = if self.interaction.enabled { t.text_on_accent_primary } else { t.text_on_accent_disabled };
        let knob_color = lerp_color(off_knob, on_knob, p);
        let knob = Rect { x: cx - KNOB_D / 2.0, y: cy - KNOB_D / 2.0, w: KNOB_D, h: KNOB_D };
        ctx.painter.fill_rounded_rect(knob, KNOB_D / 2.0, knob_color);
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
                // 空格/回车切换
                if vk == 0x20 || vk == 0x0D {
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
        (now - self.anim_start) < TOGGLE_DUR
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::CheckBox
    }
    fn accessible_name(&self) -> String {
        if self.on { self.on_text.clone() } else { self.off_text.clone() }
    }
}
