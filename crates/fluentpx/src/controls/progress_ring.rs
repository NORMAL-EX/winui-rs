//! ProgressRing（环形进度，不确定 spinner）。
//!
//! 真值来源：`controls/dev/ProgressRing/ProgressRing_themeresources.xaml`
//! * 前景 `AccentFillColorDefault`，背景透明
//! 官方为 Lottie 动画（AnimatedVisualPlayer）；此处用旋转圆弧近似：
//! 弧整体匀速旋转，弧长在 ~30°↔300° 间正弦伸缩，形成「贪吃蛇」式 spinner。

use crate::widget::*;

const SIZE: f32 = 36.0;
const ROT_PERIOD: f64 = 1.4; // 整圈旋转周期
const LEN_PERIOD: f64 = 2.0; // 弧长伸缩周期

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
        let rot = ((now % ROT_PERIOD) / ROT_PERIOD) as f32 * 360.0;
        // 弧长在 [30,300] 之间正弦伸缩
        let phase = ((now % LEN_PERIOD) / LEN_PERIOD) as f32 * std::f32::consts::TAU;
        let sweep = 30.0 + (1.0 - phase.cos()) * 0.5 * 270.0;
        // 让起点也随弧长变化前移，形成首尾交替吞吐
        let start = rot - 90.0 + (1.0 - (phase * 0.5).cos()) * 40.0;
        ctx.painter.stroke_arc(cx, cy, r, start, sweep, t.accent_fill_default(), thickness);
    }

    fn is_animating(&self, _now: f64) -> bool {
        true
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::Slider
    }
}
