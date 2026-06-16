//! NavigationView（左侧导航菜单）：可展开/收缩窗格 + 汉堡按钮 + 选中指示条 + 宽度动画。
//!
//! 真值参考：`controls/dev/NavigationView/*`。
//! * 窗格 OpenPaneLength 320 / CompactPaneLength 48（此处 demo 内开 200 / 收 48）
//! * 顶部 NavigationViewToggleButton（汉堡 `\u{E700}`）
//! * NavigationViewItem 高 ~40，选中左侧 accent 指示条（约 3×16），hover `SubtleFillColorSecondary`
//! 为在画廊里演示，整体放在一个带边框的盒子里（左窗格 + 右内容区）。

use crate::anim::{ease_out, lerp};
use crate::typography::TextStyle;
use crate::widget::*;

const OPEN_W: f32 = 200.0;
const COMPACT_W: f32 = 48.0;
const TOGGLE_H: f32 = 44.0;
const ITEM_H: f32 = 40.0;
const ICON_CX: f32 = 24.0; // 图标中心相对窗格左缘（compact 时居中）
const LABEL_X: f32 = 48.0;
const ANIM_DUR: f64 = 0.2;
const IND_W: f32 = 3.0;
const IND_H: f32 = 16.0;

pub struct NavItem {
    pub glyph: char,
    pub label: String,
}

pub struct NavigationView {
    pub items: Vec<NavItem>,
    pub selected: usize,
    pub expanded: bool,
    hovered: Option<i32>, // -1 = toggle, >=0 = item
    rect: Rect,
    anim_from: f32,
    anim_to: f32,
    anim_start: f64,
}

impl NavigationView {
    pub fn new(items: Vec<NavItem>, selected: usize, expanded: bool) -> NavigationView {
        let w = if expanded { 1.0 } else { 0.0 };
        NavigationView {
            items,
            selected,
            expanded,
            hovered: None,
            rect: Rect::default(),
            anim_from: w,
            anim_to: w,
            anim_start: -1.0,
        }
    }

    /// 默认演示项。
    pub fn demo() -> NavigationView {
        NavigationView::new(
            vec![
                NavItem { glyph: '\u{E80F}', label: "主页".into() },
                NavItem { glyph: '\u{E8B7}', label: "文件夹".into() },
                NavItem { glyph: '\u{E734}', label: "收藏".into() },
                NavItem { glyph: '\u{E713}', label: "设置".into() },
            ],
            0,
            true,
        )
    }

    fn progress(&self, now: f64) -> f32 {
        if self.anim_start < 0.0 || (now - self.anim_start) >= ANIM_DUR {
            return self.anim_to;
        }
        let t = ((now - self.anim_start) / ANIM_DUR).clamp(0.0, 1.0) as f32;
        lerp(self.anim_from, self.anim_to, ease_out(t))
    }

    fn pane_w(&self, now: f64) -> f32 {
        lerp(COMPACT_W, OPEN_W, self.progress(now))
    }

    fn toggle_rect(&self, pane_w: f32) -> Rect {
        Rect { x: self.rect.x, y: self.rect.y, w: pane_w, h: TOGGLE_H }
    }

    fn item_rect(&self, i: usize, pane_w: f32) -> Rect {
        Rect { x: self.rect.x, y: self.rect.y + TOGGLE_H + 4.0 + i as f32 * ITEM_H, w: pane_w, h: ITEM_H }
    }
}

impl Widget for NavigationView {
    fn measure(&mut self, available: Size) -> Size {
        Size { w: available.w.clamp(420.0, 600.0), h: 280.0 }
    }

    fn arrange(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn hit_test(&self, p: Point) -> bool {
        self.rect.contains(p)
    }

    fn paint(&mut self, ctx: &mut PaintCtx) {
        let t = ctx.tokens;
        let now = ctx.now;
        let pane_w = self.pane_w(now);
        let open_amt = self.progress(now); // 0..1 文字淡入

        // 整体卡片 + 边框
        ctx.painter.fill_rounded_rect(self.rect, 6.0, t.solid_bg_base);
        ctx.painter.stroke_inner(self.rect, 6.0, t.divider_stroke_default, 1.0);

        // 窗格背景（略深）
        let pane = Rect { x: self.rect.x, y: self.rect.y, w: pane_w, h: self.rect.h };
        ctx.painter.fill_rounded_rect(pane, 6.0, t.solid_bg_secondary);
        // 窗格右侧分隔线
        ctx.painter.fill_rect(Rect { x: self.rect.x + pane_w, y: self.rect.y, w: 1.0, h: self.rect.h }, t.divider_stroke_default);

        // 汉堡按钮
        let tr = self.toggle_rect(pane_w);
        if self.hovered == Some(-1) {
            ctx.painter.fill_rounded_rect(Rect { x: tr.x + 4.0, y: tr.y + 4.0, w: 40.0, h: 36.0 }, 4.0, t.subtle_fill_secondary);
        }
        let ham = Rect { x: tr.x + ICON_CX - 8.0, y: tr.center_y() - 8.0, w: 16.0, h: 16.0 };
        let _ = ctx.painter.draw_icon('\u{E700}', 16.0, ham, t.text_primary);

        // 导航项
        for i in 0..self.items.len() {
            let r = self.item_rect(i, pane_w);
            let selected = i == self.selected;
            let hovered = self.hovered == Some(i as i32);
            if selected {
                ctx.painter.fill_rounded_rect(Rect { x: r.x + 4.0, y: r.y + 2.0, w: r.w - 8.0, h: r.h - 4.0 }, 4.0, t.subtle_fill_secondary);
                let ind = Rect { x: r.x + 2.0, y: r.center_y() - IND_H / 2.0, w: IND_W, h: IND_H };
                ctx.painter.fill_rounded_rect(ind, 1.5, t.accent_fill_default());
            } else if hovered {
                ctx.painter.fill_rounded_rect(Rect { x: r.x + 4.0, y: r.y + 2.0, w: r.w - 8.0, h: r.h - 4.0 }, 4.0, t.subtle_fill_secondary);
            }
            // 图标
            let icon = Rect { x: r.x + ICON_CX - 8.0, y: r.center_y() - 8.0, w: 16.0, h: 16.0 };
            let _ = ctx.painter.draw_icon(self.items[i].glyph, 16.0, icon, t.text_primary);
            // 标签（展开时淡入）
            if open_amt > 0.05 {
                let label_rect = Rect { x: r.x + LABEL_X, y: r.y, w: (r.w - LABEL_X - 8.0).max(0.0), h: r.h };
                let _ = ctx.painter.draw_text_leading(&self.items[i].label, TextStyle::BODY, label_rect, t.text_primary.with_opacity(open_amt));
            }
        }

        // 右侧内容区
        let content = Rect { x: self.rect.x + pane_w + 1.0, y: self.rect.y, w: (self.rect.w - pane_w - 1.0).max(0.0), h: self.rect.h };
        let _ = ctx.painter.draw_text_leading(
            &self.items[self.selected].label,
            TextStyle::SUBTITLE,
            Rect { x: content.x + 24.0, y: content.y + 24.0, w: content.w - 48.0, h: 32.0 },
            t.text_primary,
        );
        let _ = ctx.painter.draw_text_leading(
            "这是导航内容区。点击左侧汉堡按钮 ☰ 可展开/收缩导航窗格。",
            TextStyle::BODY,
            Rect { x: content.x + 24.0, y: content.y + 64.0, w: content.w - 48.0, h: 24.0 },
            t.text_secondary,
        );
    }

    fn on_event(&mut self, ev: InputEvent, now: f64) -> EventResult {
        let pane_w = self.pane_w(now);
        let mut redraw = false;
        let mut animating = false;
        match ev {
            InputEvent::PointerMove(p) => {
                let mut h: Option<i32> = None;
                if self.toggle_rect(pane_w).contains(p) {
                    h = Some(-1);
                } else {
                    for i in 0..self.items.len() {
                        if self.item_rect(i, pane_w).contains(p) {
                            h = Some(i as i32);
                            break;
                        }
                    }
                }
                if h != self.hovered {
                    self.hovered = h;
                    redraw = true;
                }
            }
            InputEvent::PointerLeave => {
                if self.hovered.is_some() {
                    self.hovered = None;
                    redraw = true;
                }
            }
            InputEvent::PointerUp(p) => {
                if self.toggle_rect(pane_w).contains(p) {
                    // 切换展开/收缩
                    self.anim_from = self.progress(now);
                    self.expanded = !self.expanded;
                    self.anim_to = if self.expanded { 1.0 } else { 0.0 };
                    self.anim_start = now;
                    redraw = true;
                    animating = true;
                } else {
                    for i in 0..self.items.len() {
                        if self.item_rect(i, pane_w).contains(p) {
                            self.selected = i;
                            redraw = true;
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
        EventResult { redraw, animating }
    }

    fn is_animating(&self, now: f64) -> bool {
        self.anim_start >= 0.0 && (now - self.anim_start) < ANIM_DUR
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::List
    }
}
