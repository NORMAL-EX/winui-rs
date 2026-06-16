//! Button（普通）与 AccentButton（蓝）。
//!
//! 真值来源：`controls/dev/CommonStyles/Button_themeresources.xaml`
//! * Padding `11,5,11,6`、CornerRadius `4`、BorderThickness `1`、FontSize `14`
//! * 普通：`BackgroundSizing=InnerBorderEdge`，边框 `ControlElevationBorderBrush`（渐变）
//! * 蓝色：`BackgroundSizing=OuterBorderEdge`，背景 `AccentFillColorDefault`，
//!   文字 `TextOnAccentFillColorPrimary`(#000000)，边框 `AccentControlElevationBorderBrush`
//! * 背景过渡 `BrushTransition Duration=0:0:0.083`（83ms 线性），边框/文字离散切换
//!
//! MinHeight 取 WinUI 标准控件高 32（内容 20 行高 + 上下内边距 11 = 31，向上取 32）。

use crate::anim::ColorTransition;
use crate::color::Color;
use crate::tokens::Tokens;
use crate::typography::TextStyle;
use crate::widget::*;

const PADDING_L: f32 = 11.0;
const PADDING_T: f32 = 5.0;
const PADDING_R: f32 = 11.0;
const PADDING_B: f32 = 6.0;
const CORNER: f32 = 4.0;
const BORDER: f32 = 1.0;
const MIN_HEIGHT: f32 = 32.0;
const BG_TRANSITION: f64 = 0.083;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    Standard,
    Accent,
}

enum Border {
    None,
    Solid(Color),
    Gradient(crate::color::LinearGradient),
}

pub struct Button {
    pub text: String,
    pub style: ButtonStyle,
    pub interaction: Interaction,
    rect: Rect,
    bg: ColorTransition,
    initialized: bool,
}

impl Button {
    pub fn new(text: impl Into<String>, style: ButtonStyle) -> Button {
        Button {
            text: text.into(),
            style,
            interaction: Interaction::default(),
            rect: Rect::default(),
            bg: ColorTransition::instant(Color::TRANSPARENT),
            initialized: false,
        }
    }

    pub fn standard(text: impl Into<String>) -> Button {
        Button::new(text, ButtonStyle::Standard)
    }
    pub fn accent(text: impl Into<String>) -> Button {
        Button::new(text, ButtonStyle::Accent)
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.interaction.enabled = enabled;
    }

    fn bg_for(&self, t: &Tokens, vs: VisualState) -> Color {
        match (self.style, vs) {
            (ButtonStyle::Standard, VisualState::Normal) => t.control_fill_default,
            (ButtonStyle::Standard, VisualState::PointerOver) => t.control_fill_secondary,
            (ButtonStyle::Standard, VisualState::Pressed) => t.control_fill_tertiary,
            (ButtonStyle::Standard, VisualState::Disabled) => t.control_fill_disabled,
            (ButtonStyle::Accent, VisualState::Normal) => t.accent_fill_default(),
            (ButtonStyle::Accent, VisualState::PointerOver) => t.accent_fill_secondary(),
            (ButtonStyle::Accent, VisualState::Pressed) => t.accent_fill_tertiary(),
            (ButtonStyle::Accent, VisualState::Disabled) => t.accent_fill_disabled,
        }
    }

    fn fg_for(&self, t: &Tokens, vs: VisualState) -> Color {
        match (self.style, vs) {
            (ButtonStyle::Standard, VisualState::Pressed) => t.text_secondary,
            (ButtonStyle::Standard, VisualState::Disabled) => t.text_disabled,
            (ButtonStyle::Standard, _) => t.text_primary,
            (ButtonStyle::Accent, VisualState::Pressed) => t.text_on_accent_secondary,
            (ButtonStyle::Accent, VisualState::Disabled) => t.text_on_accent_disabled,
            (ButtonStyle::Accent, _) => t.text_on_accent_primary,
        }
    }

    fn border_for(&self, t: &Tokens, vs: VisualState) -> Border {
        match (self.style, vs) {
            (ButtonStyle::Standard, VisualState::Normal)
            | (ButtonStyle::Standard, VisualState::PointerOver) => {
                Border::Gradient(t.control_elevation_border())
            }
            (ButtonStyle::Standard, _) => Border::Solid(t.stroke_default),
            (ButtonStyle::Accent, VisualState::Normal)
            | (ButtonStyle::Accent, VisualState::PointerOver) => {
                Border::Gradient(t.accent_control_elevation_border())
            }
            // 蓝色按钮按下/禁用：边框 = ControlFillColorTransparent（透明，等效无边框）
            (ButtonStyle::Accent, _) => Border::None,
        }
    }

    /// 内边距盒（文字居中于此，匹配 11,5,11,6 的非对称内边距）。
    fn content_box(&self) -> Rect {
        Rect {
            x: self.rect.x + PADDING_L,
            y: self.rect.y + PADDING_T,
            w: (self.rect.w - PADDING_L - PADDING_R).max(0.0),
            h: (self.rect.h - PADDING_T - PADDING_B).max(0.0),
        }
    }
}

impl Widget for Button {
    fn measure(&mut self, _available: Size) -> Size {
        // 宽 = 文字宽 + 左右内边距；高 = max(32, 行高 + 上下内边距)。
        // 文字宽在 paint 阶段用真实 DWrite 测量更准；measure 给一个基于字符数的估算上界。
        let approx_text_w = self.text.chars().count() as f32 * 7.0;
        Size {
            w: approx_text_w + PADDING_L + PADDING_R,
            h: MIN_HEIGHT.max(TextStyle::BODY.line_height + PADDING_T + PADDING_B),
        }
    }

    fn arrange(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn hit_test(&self, p: Point) -> bool {
        self.rect.contains(p)
    }

    fn paint(&mut self, ctx: &mut PaintCtx) {
        let t = ctx.tokens;
        let vs = self.interaction.visual_state();

        // 每帧按「当前状态 + 当前主题」重新求目标背景色：目标一变就过渡过去（83ms）。
        // 这样既覆盖交互状态切换（hover/press/release/leave 后恢复），也覆盖主题切换
        // （token 变化时即使 vs 未变也会重新着色），避免缓存的旧色卡住不恢复。
        let target = self.bg_for(t, vs);
        if !self.initialized {
            self.bg = ColorTransition::instant(target);
            self.initialized = true;
        } else if target != self.bg.to {
            self.bg.retarget(target, ctx.now, BG_TRANSITION);
        }
        let bg = self.bg.value(ctx.now);
        let r = self.rect;

        match self.style {
            // OuterBorderEdge：背景铺满外缘，边框压在其上。
            ButtonStyle::Accent => {
                ctx.painter.fill_rounded_rect(r, CORNER, bg);
            }
            // InnerBorderEdge：背景只铺到边框内沿。
            ButtonStyle::Standard => {
                ctx.painter
                    .fill_rounded_rect(r.inset(BORDER), (CORNER - BORDER).max(0.0), bg);
            }
        }

        match self.border_for(t, vs) {
            Border::None => {}
            Border::Solid(c) => ctx.painter.stroke_inner(r, CORNER, c, BORDER),
            Border::Gradient(g) => {
                let _ = ctx.painter.stroke_inner_gradient(r, CORNER, &g, BORDER);
            }
        }

        let fg = self.fg_for(t, vs);
        let _ = ctx
            .painter
            .draw_text_centered(&self.text, TextStyle::BODY, self.content_box(), fg);
    }

    fn on_event(&mut self, ev: InputEvent, _now: f64) -> EventResult {
        if !self.interaction.enabled {
            return EventResult::NONE;
        }
        let before = self.interaction;
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
                } else {
                    self.interaction.focused = false;
                }
            }
            InputEvent::PointerUp(_) => self.interaction.pressed = false,
            _ => {}
        }
        let changed = before.visual_state() != self.interaction.visual_state();
        EventResult { redraw: changed, animating: changed }
    }

    fn is_animating(&self, now: f64) -> bool {
        self.bg.is_active(now)
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::Button
    }
    fn accessible_name(&self) -> String {
        self.text.clone()
    }
}
