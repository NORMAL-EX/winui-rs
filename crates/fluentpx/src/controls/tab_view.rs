//! TabView（选择夹）：选中标签的形状 + 分隔线。
//!
//! 真值来源：`controls/dev/TabView/TabView_themeresources.xaml`
//! * TabViewItemHeaderPadding `8,3,4,3`，MinWidth 100，MaxWidth 240
//! * 选中背景 `SolidBackgroundFillColorTertiary`，未选中背景透明
//! * 前景 `TextFillColorSecondary`（选中时用 Primary 更清晰，与 WinUI 行为一致）
//! * 标签间分隔线 `DividerStrokeColorDefault`
//! * 选中标签上方两角圆角（OverlayCornerRadius/ControlCornerRadius 顶部）
//!
//! 关闭按钮（每个标签右侧的 ✕）几何已预留位置，先画占位区，标注后续细化。

use crate::widget::*;

const TAB_H: f32 = 32.0;
const TAB_MIN_W: f32 = 100.0;
const PAD_L: f32 = 8.0;
const CORNER_TOP: f32 = 7.0; // 选中标签顶部圆角（WinUI TabView 用较大圆角）

pub struct TabView {
    pub tabs: Vec<String>,
    pub selected: usize,
    hovered: Option<usize>,
    rect: Rect,
}

impl TabView {
    pub fn new(tabs: Vec<String>, selected: usize) -> TabView {
        TabView { tabs, selected, hovered: None, rect: Rect::default() }
    }

    fn tab_rect(&self, i: usize) -> Rect {
        Rect { x: self.rect.x + i as f32 * TAB_MIN_W, y: self.rect.y, w: TAB_MIN_W, h: TAB_H }
    }

    fn index_at(&self, p: Point) -> Option<usize> {
        if !self.rect.contains(p) {
            return None;
        }
        let i = ((p.x - self.rect.x) / TAB_MIN_W).floor() as i32;
        if i >= 0 && (i as usize) < self.tabs.len() {
            Some(i as usize)
        } else {
            None
        }
    }
}

impl Widget for TabView {
    fn measure(&mut self, _available: Size) -> Size {
        Size { w: TAB_MIN_W * self.tabs.len() as f32, h: TAB_H }
    }

    fn arrange(&mut self, rect: Rect) {
        self.rect = rect;
    }

    fn hit_test(&self, p: Point) -> bool {
        self.rect.contains(p)
    }

    fn paint(&mut self, ctx: &mut PaintCtx) {
        let t = ctx.tokens;

        for i in 0..self.tabs.len() {
            let r = self.tab_rect(i);
            let selected = i == self.selected;
            let hovered = self.hovered == Some(i);

            if selected {
                // 选中：实底 + 顶部两角圆角（用圆角矩形向下溢出隐藏底部圆角的近似）。
                let shape = Rect { x: r.x, y: r.y, w: r.w, h: r.h + CORNER_TOP };
                ctx.painter.fill_rounded_rect(shape, CORNER_TOP, t.solid_bg_tertiary);
            } else if hovered {
                ctx.painter.fill_rounded_rect(r.inset(2.0), 4.0, t.subtle_fill_secondary);
            }

            // 分隔线：未选中标签之间画竖线（避开选中项两侧）。
            if i + 1 < self.tabs.len() && !selected && i + 1 != self.selected {
                let x = r.right();
                let sep = Rect { x: x - 0.5, y: r.y + 8.0, w: 1.0, h: r.h - 16.0 };
                ctx.painter.fill_rect(sep, t.divider_stroke_default);
            }

            let fg = if selected { t.text_primary } else { t.text_secondary };
            let text_rect = Rect { x: r.x + PAD_L, y: r.y, w: r.w - PAD_L - 24.0, h: r.h };
            let _ = ctx.painter.draw_text_leading(&self.tabs[i], crate::typography::TextStyle::BODY, text_rect, fg);
        }

        // 选中标签下方的内容区分隔（TabView 内容面板顶边），与选中标签同色衔接。
        let content_top = Rect { x: self.rect.x, y: self.rect.bottom(), w: self.rect.w.max(TAB_MIN_W * 3.0), h: 1.0 };
        ctx.painter.fill_rect(content_top, t.divider_stroke_default);
    }

    fn on_event(&mut self, ev: InputEvent, _now: f64) -> EventResult {
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
                if self.hovered.is_some() {
                    self.hovered = None;
                    redraw = true;
                }
            }
            InputEvent::PointerDown(p) => {
                if let Some(i) = self.index_at(p) {
                    if i != self.selected {
                        self.selected = i;
                        redraw = true;
                    }
                }
            }
            _ => {}
        }
        EventResult { redraw, animating: false }
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::Tab
    }
}
