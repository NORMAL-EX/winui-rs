//! NavigationView（左侧导航菜单）：可展开/收缩窗格 + 汉堡按钮 + 选中指示条 + 宽度动画。
//!
//! 真值参考：`controls/dev/NavigationView/*`。
//! * 窗格 OpenPaneLength 320 / CompactPaneLength 48（此处 demo 内开 200 / 收 48）
//! * 顶部 NavigationViewToggleButton（汉堡 `\u{E700}`）
//! * NavigationViewItem 高 ~40，选中左侧 accent 指示条（约 3×16），hover `SubtleFillColorSecondary`
//! 为在画廊里演示，整体放在一个带边框的盒子里（左窗格 + 右内容区）。

use crate::anim::{cubic_bezier, ease_out, lerp};
use crate::gfx::Icon;
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
// 选中指示条动画（源码 PlayIndicatorAnimations）：总 600ms，前缘 0.333 到位、后缘缓出。
const SEL_DUR: f64 = 0.6;
// 页面进入动画（Entrance/Slide）：下移淡入。
const PAGE_DUR: f64 = 0.3;
const PAGE_OFFSET: f32 = 16.0;

pub struct NavItem {
    pub icon: Icon,
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
    // 选中指示条 + 页面切换动画
    prev_selected: usize,
    sel_start: f64,
    /// 应用外壳模式：铺满整个 rect、无卡片边框，内容区交给宿主应用绘制。
    pub app_shell: bool,
    /// 是否绘制内置演示内容（gallery 用 true；应用外壳用 false 由宿主填内容）。
    pub show_demo_content: bool,
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
            prev_selected: selected,
            sel_start: -1.0,
            app_shell: false,
            show_demo_content: true,
        }
    }

    /// 作为应用外壳：铺满窗口、内容区由宿主填充。
    pub fn shell(items: Vec<NavItem>, selected: usize) -> NavigationView {
        let mut n = NavigationView::new(items, selected, true);
        n.app_shell = true;
        n.show_demo_content = false;
        n
    }

    /// 当前内容区矩形（窗格右侧），供应用外壳模式下宿主绘制页面。
    pub fn content_area(&self, now: f64) -> Rect {
        let pane_w = self.pane_w(now);
        Rect { x: self.rect.x + pane_w + 1.0, y: self.rect.y, w: (self.rect.w - pane_w - 1.0).max(0.0), h: self.rect.h }
    }

    /// 默认演示项。
    pub fn demo() -> NavigationView {
        NavigationView::new(
            vec![
                NavItem { icon: Icon::Home, label: "主页".into() },
                NavItem { icon: Icon::Folder, label: "文件夹".into() },
                NavItem { icon: Icon::Star, label: "收藏".into() },
                NavItem { icon: Icon::Settings, label: "设置".into() },
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

        // 整体卡片 + 边框（应用外壳模式下铺满、无卡片边框）
        if !self.app_shell {
            ctx.painter.fill_rounded_rect(self.rect, 6.0, t.solid_bg_base);
            ctx.painter.stroke_inner(self.rect, 6.0, t.divider_stroke_default, 1.0);
        }

        // 窗格背景（略深）
        let pane = Rect { x: self.rect.x, y: self.rect.y, w: pane_w, h: self.rect.h };
        if self.app_shell {
            ctx.painter.fill_rect(pane, t.solid_bg_secondary);
        } else {
            ctx.painter.fill_rounded_rect(pane, 6.0, t.solid_bg_secondary);
        }
        // 窗格右侧分隔线
        ctx.painter.fill_rect(Rect { x: self.rect.x + pane_w, y: self.rect.y, w: 1.0, h: self.rect.h }, t.divider_stroke_default);

        // 汉堡按钮
        let tr = self.toggle_rect(pane_w);
        if self.hovered == Some(-1) {
            ctx.painter.fill_rounded_rect(Rect { x: tr.x + 4.0, y: tr.y + 4.0, w: 40.0, h: 36.0 }, 4.0, t.subtle_fill_secondary);
        }
        let ham = Rect { x: tr.x + ICON_CX - 8.0, y: tr.center_y() - 8.0, w: 16.0, h: 16.0 };
        ctx.painter.draw_glyph(Icon::Hamburger, ham, t.text_primary);

        // 导航项（背景 / hover / 图标 / 标签）——选中指示条单独做动画绘制。
        for i in 0..self.items.len() {
            let r = self.item_rect(i, pane_w);
            let selected = i == self.selected;
            let hovered = self.hovered == Some(i as i32);
            if selected || hovered {
                ctx.painter.fill_rounded_rect(Rect { x: r.x + 4.0, y: r.y + 2.0, w: r.w - 8.0, h: r.h - 4.0 }, 4.0, t.subtle_fill_secondary);
            }
            let icon = Rect { x: r.x + ICON_CX - 8.0, y: r.center_y() - 8.0, w: 16.0, h: 16.0 };
            ctx.painter.draw_glyph(self.items[i].icon, icon, t.text_primary);
            if open_amt > 0.05 {
                let label_rect = Rect { x: r.x + LABEL_X, y: r.y, w: (r.w - LABEL_X - 8.0).max(0.0), h: r.h };
                let _ = ctx.painter.draw_text_leading(&self.items[i].label, TextStyle::BODY, label_rect, t.text_primary.with_opacity(open_amt));
            }
        }

        // 选中指示条：拉伸滑动（源码 PlayIndicatorAnimations：前缘快、后缘缓，中途拉伸）。
        let cur_c = self.item_rect(self.selected, pane_w).center_y();
        let prev_c = self.item_rect(self.prev_selected, pane_w).center_y();
        let (top, bot) = indicator_edges(self.sel_start, prev_c, cur_c, now);
        let ind = Rect { x: self.rect.x + 2.0, y: top, w: IND_W, h: (bot - top).max(1.0) };
        ctx.painter.fill_rounded_rect(ind, IND_W / 2.0, t.accent_fill_default());

        // 右侧内容区演示（仅 gallery 演示用；应用外壳由宿主在 content_area 自行绘制）。
        if self.show_demo_content {
            let content = Rect { x: self.rect.x + pane_w + 1.0, y: self.rect.y, w: (self.rect.w - pane_w - 1.0).max(0.0), h: self.rect.h };
            ctx.painter.push_clip(content);
            let pp = if self.sel_start < 0.0 {
                1.0
            } else {
                ease_out(((now - self.sel_start) / PAGE_DUR).clamp(0.0, 1.0) as f32)
            };
            let dy = (1.0 - pp) * PAGE_OFFSET;
            let _ = ctx.painter.draw_text_leading(
                &self.items[self.selected].label,
                TextStyle::SUBTITLE,
                Rect { x: content.x + 24.0, y: content.y + 24.0 + dy, w: content.w - 48.0, h: 32.0 },
                t.text_primary.with_opacity(pp),
            );
            let _ = ctx.painter.draw_text_leading(
                "这是导航内容区。点左侧汉堡 ☰ 可展开/收缩；切换项有指示条滑动 + 页面进入动画。",
                TextStyle::BODY,
                Rect { x: content.x + 24.0, y: content.y + 64.0 + dy, w: content.w - 48.0, h: 24.0 },
                t.text_secondary.with_opacity(pp),
            );
            ctx.painter.pop_clip();
        }
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
                            if i != self.selected {
                                self.prev_selected = self.selected;
                                self.selected = i;
                                self.sel_start = now; // 触发指示条滑动 + 页面进入动画
                                animating = true;
                            }
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
        (self.anim_start >= 0.0 && (now - self.anim_start) < ANIM_DUR)
            || (self.sel_start >= 0.0 && (now - self.sel_start) < SEL_DUR)
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::List
    }
}

/// 选中指示条上下边随时间的位置（拉伸滑动，源码 PlayIndicatorAnimations）。
fn indicator_edges(sel_start: f64, prev_c: f32, cur_c: f32, now: f64) -> (f32, f32) {
    let old_top = prev_c - IND_H / 2.0;
    let old_bot = prev_c + IND_H / 2.0;
    let new_top = cur_c - IND_H / 2.0;
    let new_bot = cur_c + IND_H / 2.0;
    if sel_start < 0.0 {
        return (new_top, new_bot);
    }
    let t = ((now - sel_start) / SEL_DUR).clamp(0.0, 1.0) as f32;
    if t >= 1.0 {
        return (new_top, new_bot);
    }
    // 两段式（对应源码 Offset 在 0.333 步进 + Scale 先涨后落）：
    //   阶段1 (0~0.333)：领先边伸向目标(frame1 缓动)，拖尾边按住不动 → 拉长；
    //   阶段2 (0.333~1)：领先边到位按住，拖尾边收向目标(frame2 缓动) → 收回。
    let p1 = (t / 0.333).min(1.0);
    let p2 = ((t - 0.333) / 0.667).clamp(0.0, 1.0);
    let stretch = cubic_bezier(0.9, 0.1, 1.0, 0.2, p1); // frame1
    let settle = cubic_bezier(0.1, 0.9, 0.2, 1.0, p2); // frame2
    if cur_c >= prev_c {
        // 下移：底边领先、顶边收尾
        let bottom = lerp(old_bot, new_bot, stretch);
        let top = lerp(old_top, new_top, settle);
        (top, bottom)
    } else {
        // 上移：顶边领先、底边收尾
        let top = lerp(old_top, new_top, stretch);
        let bottom = lerp(old_bot, new_bot, settle);
        (top, bottom)
    }
}
