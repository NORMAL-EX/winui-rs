//! ProgressRing（环形进度，不确定 spinner）。
//!
//! 真值来源：`controls/dev/ProgressRing/ProgressRing_themeresources.xaml`
//! * 前景 `AccentFillColorDefault`，背景透明
//! 官方为 Lottie 动画（AnimatedVisualPlayer）；此处用旋转圆弧近似：
//! 弧整体匀速旋转，弧长在 ~30°↔300° 间正弦伸缩，形成「贪吃蛇」式 spinner。

use crate::widget::*;

const SIZE: f32 = 36.0;
const CYCLE: f64 = 1.4; // 一次「伸→缩」周期
const SPAN: f32 = 290.0; // 弧最大张角（留 ~70° 缺口）

pub struct ProgressRing {
    rect: Rect,
}

impl ProgressRing {
    pub fn new() -> ProgressRing {
        ProgressRing { rect: Rect::default() }
    }
}

impl Default for ProgressRing {
    fn default() -> Self {
        ProgressRing::new()
    }
}

impl Widget for ProgressRing {
    fn measure(&mut self, _available: Size) -> Size {
        Size { w: SIZE, h: SIZE }
    }

    fn arrange(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn hit_test(&self, _p: Point) -> bool {
        false
    }

    fn paint(&mut self, ctx: &mut PaintCtx) {
        let t = ctx.tokens;
        let cx = self.rect.x + SIZE / 2.0;
        let cy = self.rect.center_y();
        let thickness = SIZE * 0.09;
        let r = SIZE / 2.0 - thickness;

        let now = ctx.now;
        // 头/尾各按 easeInOut 推进，前半程头领先（伸长），后半程尾跟上（收缩）。
        let cycle = ((now % CYCLE) / CYCLE) as f32;
        let ease = |x: f32| -> f32 {
            let x = x.clamp(0.0, 1.0);
            if x < 0.5 { 4.0 * x * x * x } else { 1.0 - (-2.0 * x + 2.0).powi(3) / 2.0 }
        };
        let head = ease((cycle * 2.0).min(1.0)) * SPAN;
        let tail = ease((cycle * 2.0 - 1.0).max(0.0)) * SPAN;
        // 连续基础旋转（每 ~2.6s 一圈）+ 尾部推进，整体匀速转动。
        let base = ((now % 2.6) / 2.6) as f32 * 360.0;
        let start = base + tail - 90.0;
        let sweep = (head - tail).clamp(14.0, SPAN);
        ctx.painter.stroke_arc(cx, cy, r, start, sweep, t.accent_fill_default(), thickness);
    }

    fn is_animating(&self, _now: f64) -> bool {
        true
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::Slider
    }
}
