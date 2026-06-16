//! Direct2D / DirectWrite 渲染引擎。
//!
//! 设计要点（对应规格第 5 节「像素对齐隐藏项」）：
//! * **坐标模型**：渲染目标 DPI 固定为 96，所有逻辑坐标在 [`Painter`] 内 × `scale`
//!   换算成设备像素后再绘制，从而对 1px 边框/分隔线做显式设备像素取整。
//! * **文字 AA / gamma**：[`TEXT_AA_MODE`] 与自定义 `IDWriteRenderingParams` 的 gamma
//!   集中可调，对照参考截图把字形边缘灰度调到一致。
//! * **1px 描边内缩**：D2D 描边以路径中心线为准，[`Painter::stroke_inner`] 把矩形内缩
//!   半个描边宽，使边框落在内沿（WinUI 的 InnerBorderEdge 行为）。
//! * **渐变边框**：[`Painter::fill_with_gradient_border`] 用 `ID2D1LinearGradientBrush`
//!   按源码 GradientStops 真值绘制立体高光边。

use windows::core::{Interface, Result};
use windows::Foundation::Numerics::Matrix3x2;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::*;

use crate::color::{Color, LinearGradient};
use crate::typography::{create_text_format, TextStyle};
use crate::widget::{Point, Rect, Size};

/// 文字抗锯齿模式。WinUI 在合成层对文字用灰度 AA；如对照参考偏色可切到 CLEARTYPE。
pub const TEXT_AA_MODE: D2D1_TEXT_ANTIALIAS_MODE = D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE;
/// 文字 gamma（自定义渲染参数）。默认 1.8 接近 Windows 文本渲染；偏色时在此微调。
pub const TEXT_GAMMA: f32 = 1.8;
/// ClearType 对比度增强系数。
pub const TEXT_ENHANCED_CONTRAST: f32 = 0.5;

/// 内置矢量图标（用 D2D 几何绘制，零字体依赖，Fluent 线性风格）。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    ChevronDown,
    Hamburger,
    Home,
    Folder,
    Star,
    Settings,
    Info,
    Success,
    Warning,
    Error,
}

/// 进程级 D2D/DWrite 工厂（与窗口无关，可全局复用）。
pub struct Gfx {
    pub d2d: ID2D1Factory,
    pub dwrite: IDWriteFactory,
    /// 实际使用的图标字体族（Segoe Fluent Icons / Segoe MDL2 Assets）。
    pub icon_font: String,
}

/// 查询系统字体集中是否存在某字体族。
fn font_family_exists(dwrite: &IDWriteFactory, name: &str) -> bool {
    unsafe {
        let mut collection: Option<IDWriteFontCollection> = None;
        if dwrite.GetSystemFontCollection(&mut collection, false).is_err() {
            return false;
        }
        let Some(collection) = collection else { return false };
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut index = 0u32;
        let mut exists = windows::Win32::Foundation::BOOL(0);
        if collection
            .FindFamilyName(windows::core::PCWSTR(wide.as_ptr()), &mut index, &mut exists)
            .is_err()
        {
            return false;
        }
        exists.as_bool()
    }
}

impl Gfx {
    pub fn new() -> Result<Gfx> {
        unsafe {
            let d2d: ID2D1Factory =
                D2D1CreateFactory::<ID2D1Factory>(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let dwrite: IDWriteFactory = DWriteCreateFactory::<IDWriteFactory>(DWRITE_FACTORY_TYPE_SHARED)?;
            // 图标字体：优先 Win11 的「Segoe Fluent Icons」，缺失（如 Win10）回退「Segoe MDL2 Assets」。
            let icon_font = if font_family_exists(&dwrite, "Segoe Fluent Icons") {
                "Segoe Fluent Icons".to_string()
            } else {
                "Segoe MDL2 Assets".to_string()
            };
            Ok(Gfx { d2d, dwrite, icon_font })
        }
    }

    /// 为窗口创建/绑定 HWND 渲染目标。`size_px` 为客户区设备像素尺寸。
    pub fn create_surface(&self, hwnd: HWND, width_px: u32, height_px: u32) -> Result<Surface> {
        let rt_props = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
            },
            // DPI 固定 96：逻辑坐标由 Painter 显式 × scale，便于像素取整。
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
            hwnd,
            pixelSize: D2D_SIZE_U { width: width_px.max(1), height: height_px.max(1) },
            presentOptions: D2D1_PRESENT_OPTIONS_NONE,
        };
        let rt = unsafe { self.d2d.CreateHwndRenderTarget(&rt_props, &hwnd_props)? };
        unsafe { rt.SetTextAntialiasMode(TEXT_AA_MODE) };

        // 自定义文字渲染参数：锁定 gamma，避免随显示器默认值漂移，保证比对可复现。
        if let Ok(params) = unsafe {
            self.dwrite.CreateCustomRenderingParams(
                TEXT_GAMMA,
                TEXT_ENHANCED_CONTRAST,
                0.0,
                DWRITE_PIXEL_GEOMETRY_FLAT,
                DWRITE_RENDERING_MODE_NATURAL,
            )
        } {
            unsafe { rt.SetTextRenderingParams(&params) };
        }

        Ok(Surface { rt, brush: None, d2d: self.d2d.clone() })
    }
}

/// 与单个窗口绑定的渲染表面。
pub struct Surface {
    rt: ID2D1HwndRenderTarget,
    /// 复用的纯色画刷（每次填充前 SetColor，省去反复创建）。
    brush: Option<ID2D1SolidColorBrush>,
    /// D2D 工厂（创建路径几何用）。
    d2d: ID2D1Factory,
}

impl Surface {
    /// 客户区尺寸变化时调整后备缓冲。
    pub fn resize(&self, width_px: u32, height_px: u32) -> Result<()> {
        unsafe {
            self.rt.Resize(&D2D_SIZE_U { width: width_px.max(1), height: height_px.max(1) })
        }
    }

    /// 开一帧。返回的 [`Painter`] 在 drop 时不自动结束，调用方需配对 [`Painter::end`]。
    pub fn begin<'a>(&'a mut self, dwrite: &'a IDWriteFactory, icon_font: &'a str, scale: f32) -> Result<Painter<'a>> {
        if self.brush.is_none() {
            let b = unsafe {
                self.rt.CreateSolidColorBrush(&Color::TRANSPARENT.d2d(), None)?
            };
            self.brush = Some(b);
        }
        unsafe { self.rt.BeginDraw() };
        Ok(Painter {
            rt: &self.rt,
            d2d: &self.d2d,
            dwrite,
            brush: self.brush.as_ref().unwrap(),
            icon_font,
            scale,
        })
    }
}

/// 一帧的绘制器。输入坐标一律是逻辑像素，内部 × `scale` 取整到设备像素。
pub struct Painter<'a> {
    rt: &'a ID2D1HwndRenderTarget,
    d2d: &'a ID2D1Factory,
    dwrite: &'a IDWriteFactory,
    brush: &'a ID2D1SolidColorBrush,
    icon_font: &'a str,
    scale: f32,
}

impl<'a> Painter<'a> {
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// 逻辑→设备像素并取整（边缘对齐）。
    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// 不取整的逻辑→设备（用于半像素描边定位）。
    fn dev(&self, v: f32) -> f32 {
        v * self.scale
    }

    fn dev_rect(&self, r: Rect) -> D2D_RECT_F {
        D2D_RECT_F {
            left: self.px(r.x),
            top: self.px(r.y),
            right: self.px(r.right()),
            bottom: self.px(r.bottom()),
        }
    }

    fn set_brush(&self, color: Color) {
        unsafe { self.brush.SetColor(&color.d2d()) };
    }

    /// 整屏清成某颜色。
    pub fn clear(&self, color: Color) {
        unsafe { self.rt.Clear(Some(&color.d2d())) };
    }

    /// 实心矩形。
    pub fn fill_rect(&self, r: Rect, color: Color) {
        if color.a == 0 {
            return;
        }
        self.set_brush(color);
        unsafe { self.rt.FillRectangle(&self.dev_rect(r), self.brush) };
    }

    /// 实心圆角矩形（`radius` 为逻辑像素角半径）。
    pub fn fill_rounded_rect(&self, r: Rect, radius: f32, color: Color) {
        if color.a == 0 {
            return;
        }
        self.set_brush(color);
        let rr = D2D1_ROUNDED_RECT {
            rect: self.dev_rect(r),
            radiusX: self.px(radius),
            radiusY: self.px(radius),
        };
        unsafe { self.rt.FillRoundedRectangle(&rr, self.brush) };
    }

    /// 在矩形**内沿**描 1（或 n）逻辑像素边框（InnerBorderEdge）。
    /// 描边中心线内缩 thickness/2 设备像素，使外缘与矩形外缘对齐、得到清晰边。
    pub fn stroke_inner(&self, r: Rect, radius: f32, color: Color, thickness_logical: f32) {
        if color.a == 0 {
            return;
        }
        self.set_brush(color);
        let t = self.dev(thickness_logical).max(1.0);
        let half = t / 2.0;
        let rect = D2D_RECT_F {
            left: self.px(r.x) + half,
            top: self.px(r.y) + half,
            right: self.px(r.right()) - half,
            bottom: self.px(r.bottom()) - half,
        };
        let rr = D2D1_ROUNDED_RECT {
            rect,
            radiusX: (self.px(radius) - half).max(0.0),
            radiusY: (self.px(radius) - half).max(0.0),
        };
        unsafe { self.rt.DrawRoundedRectangle(&rr, self.brush, t, None) };
    }

    /// 用线性渐变在内沿描边（普通/蓝色按钮的立体高光边）。
    pub fn stroke_inner_gradient(
        &self,
        r: Rect,
        radius: f32,
        gradient: &LinearGradient,
        thickness_logical: f32,
    ) -> Result<()> {
        let brush = self.create_gradient_brush(r, gradient)?;
        let t = self.dev(thickness_logical).max(1.0);
        let half = t / 2.0;
        let rect = D2D_RECT_F {
            left: self.px(r.x) + half,
            top: self.px(r.y) + half,
            right: self.px(r.right()) - half,
            bottom: self.px(r.bottom()) - half,
        };
        let rr = D2D1_ROUNDED_RECT {
            rect,
            radiusX: (self.px(radius) - half).max(0.0),
            radiusY: (self.px(radius) - half).max(0.0),
        };
        unsafe { self.rt.DrawRoundedRectangle(&rr, &brush, t, None) };
        Ok(())
    }

    /// 构建一个映射到矩形 `r` 的线性渐变画刷。
    /// 处理 XAML 的 Absolute 映射（端点为 DIP，相对矩形左上）与 ScaleY=-1 翻转。
    fn create_gradient_brush(&self, r: Rect, g: &LinearGradient) -> Result<ID2D1LinearGradientBrush> {
        let stops: Vec<D2D1_GRADIENT_STOP> = g
            .stops
            .iter()
            .map(|s| D2D1_GRADIENT_STOP { position: s.offset, color: s.color.d2d() })
            .collect();
        let collection = unsafe {
            self.rt.CreateGradientStopCollection(
                &stops,
                D2D1_GAMMA_2_2,
                D2D1_EXTEND_MODE_CLAMP,
            )?
        };

        // 端点（设备像素）。Absolute：相对矩形左上的 DIP；否则相对包围盒比例。
        let top = self.dev(r.y);
        let bottom = self.dev(r.bottom());
        let (mut p0, mut p1) = if g.absolute {
            (
                D2D_POINT_2F { x: self.dev(r.x) + self.dev(g.start.0), y: top + self.dev(g.start.1) },
                D2D_POINT_2F { x: self.dev(r.x) + self.dev(g.end.0), y: top + self.dev(g.end.1) },
            )
        } else {
            (
                D2D_POINT_2F {
                    x: self.dev(r.x) + self.dev(r.w) * g.start.0,
                    y: top + (bottom - top) * g.start.1,
                },
                D2D_POINT_2F {
                    x: self.dev(r.x) + self.dev(r.w) * g.end.0,
                    y: top + (bottom - top) * g.end.1,
                },
            )
        };
        if g.flip_y {
            // 绕矩形竖直中心反射：y' = top + bottom - y。
            p0.y = top + bottom - p0.y;
            p1.y = top + bottom - p1.y;
        }

        let props = D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES { startPoint: p0, endPoint: p1 };
        unsafe { self.rt.CreateLinearGradientBrush(&props, None, &collection) }
    }

    /// 文本测量（逻辑像素）。用于按钮等按内容定尺寸。
    pub fn measure_text(&self, text: &str, style: TextStyle) -> Result<Size> {
        let format = create_text_format(self.dwrite, style, self.scale)?;
        let wide: Vec<u16> = text.encode_utf16().collect();
        let layout = unsafe {
            self.dwrite.CreateTextLayout(&wide, &format, f32::MAX, f32::MAX)?
        };
        let mut m = DWRITE_TEXT_METRICS::default();
        unsafe { layout.GetMetrics(&mut m)? };
        // metrics 为设备像素（因字号已 × scale），换回逻辑像素。
        Ok(Size { w: m.width / self.scale, h: m.height / self.scale })
    }

    /// 在矩形内绘制单行文本，水平/垂直居中（用于按钮标签等）。
    pub fn draw_text_centered(&self, text: &str, style: TextStyle, r: Rect, color: Color) -> Result<()> {
        self.draw_text(text, style, r, color, DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_PARAGRAPH_ALIGNMENT_CENTER)
    }

    /// 左对齐、垂直居中（用于段落标题、列表项文字等）。
    pub fn draw_text_leading(&self, text: &str, style: TextStyle, r: Rect, color: Color) -> Result<()> {
        self.draw_text(text, style, r, color, DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_PARAGRAPH_ALIGNMENT_CENTER)
    }

    /// 在矩形内绘制单行文本，指定水平/垂直对齐。
    pub fn draw_text(
        &self,
        text: &str,
        style: TextStyle,
        r: Rect,
        color: Color,
        h_align: DWRITE_TEXT_ALIGNMENT,
        v_align: DWRITE_PARAGRAPH_ALIGNMENT,
    ) -> Result<()> {
        let format = create_text_format(self.dwrite, style, self.scale)?;
        unsafe {
            format.SetTextAlignment(h_align)?;
            format.SetParagraphAlignment(v_align)?;
        }
        self.set_brush(color);
        let wide: Vec<u16> = text.encode_utf16().collect();
        let layout_rect = self.dev_rect(r);
        unsafe {
            self.rt.DrawText(
                &wide,
                &format,
                &layout_rect,
                self.brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            )
        };
        Ok(())
    }

    /// 绘制一个图标字形（如 ComboBox 的 ChevronDown `\u{E70D}`），居中于 `r`。
    /// 用 `Segoe MDL2 Assets`（Win10/11 均自带；Win11 独有的 `Segoe Fluent Icons`
    /// 在 Win10 缺失会渲染成缺字方块，故统一用 MDL2，码位兼容）。`size` 为逻辑像素字号。
    pub fn draw_icon(&self, glyph: char, size: f32, r: Rect, color: Color) -> Result<()> {
        let family = windows::core::HSTRING::from(self.icon_font);
        let locale = windows::core::HSTRING::from("en-US");
        let format = unsafe {
            self.dwrite.CreateTextFormat(
                &family,
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                size * self.scale,
                &locale,
            )?
        };
        unsafe {
            format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
            format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        }
        self.set_brush(color);
        let mut buf = [0u16; 2];
        let wide = glyph.encode_utf16(&mut buf);
        let layout_rect = self.dev_rect(r);
        unsafe {
            self.rt.DrawText(
                wide,
                &format,
                &layout_rect,
                self.brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            )
        };
        Ok(())
    }

    /// 直线（逻辑坐标），用于分隔线/简单图形。
    pub fn draw_line(&self, x0: f32, y0: f32, x1: f32, y1: f32, color: Color, width: f32) {
        self.set_brush(color);
        let p0 = D2D_POINT_2F { x: self.dev(x0), y: self.dev(y0) };
        let p1 = D2D_POINT_2F { x: self.dev(x1), y: self.dev(y1) };
        unsafe { self.rt.DrawLine(p0, p1, self.brush, self.dev(width).max(1.0), None) };
    }

    // ———————————————— 矢量图标（零字体依赖，避免缺字方块）————————————————

    fn round_stroke(&self) -> Option<ID2D1StrokeStyle> {
        let props = D2D1_STROKE_STYLE_PROPERTIES {
            startCap: D2D1_CAP_STYLE_ROUND,
            endCap: D2D1_CAP_STYLE_ROUND,
            dashCap: D2D1_CAP_STYLE_ROUND,
            lineJoin: D2D1_LINE_JOIN_ROUND,
            miterLimit: 10.0,
            dashStyle: D2D1_DASH_STYLE_SOLID,
            dashOffset: 0.0,
        };
        unsafe { self.d2d.CreateStrokeStyle(&props, None).ok() }
    }

    fn build_path(&self, pts: &[(f32, f32)], closed: bool, filled: bool) -> Result<ID2D1PathGeometry> {
        let geo = unsafe { self.d2d.CreatePathGeometry()? };
        let sink = unsafe { geo.Open()? };
        let dev: Vec<D2D_POINT_2F> = pts.iter().map(|&(x, y)| D2D_POINT_2F { x: self.dev(x), y: self.dev(y) }).collect();
        unsafe {
            sink.BeginFigure(dev[0], if filled { D2D1_FIGURE_BEGIN_FILLED } else { D2D1_FIGURE_BEGIN_HOLLOW });
            sink.AddLines(&dev[1..]);
            sink.EndFigure(if closed { D2D1_FIGURE_END_CLOSED } else { D2D1_FIGURE_END_OPEN });
            sink.Close()?;
        }
        Ok(geo)
    }

    /// 折线描边（逻辑坐标），圆头圆角，用于 chevron / 勾 / 汉堡等。
    pub fn stroke_polyline(&self, pts: &[(f32, f32)], color: Color, width: f32) {
        if pts.len() < 2 {
            return;
        }
        self.set_brush(color);
        if let Ok(geo) = self.build_path(pts, false, false) {
            let ss = self.round_stroke();
            unsafe { self.rt.DrawGeometry(&geo, self.brush, self.dev(width).max(1.0), ss.as_ref()) };
        }
    }

    /// 闭合多边形描边。
    pub fn stroke_polygon(&self, pts: &[(f32, f32)], color: Color, width: f32) {
        if pts.len() < 2 {
            return;
        }
        self.set_brush(color);
        if let Ok(geo) = self.build_path(pts, true, false) {
            let ss = self.round_stroke();
            unsafe { self.rt.DrawGeometry(&geo, self.brush, self.dev(width).max(1.0), ss.as_ref()) };
        }
    }

    /// 闭合多边形填充（用于星形等）。
    pub fn fill_polygon(&self, pts: &[(f32, f32)], color: Color) {
        if pts.len() < 3 {
            return;
        }
        self.set_brush(color);
        if let Ok(geo) = self.build_path(pts, true, true) {
            unsafe { self.rt.FillGeometry(&geo, self.brush, None) };
        }
    }

    pub fn stroke_circle(&self, cx: f32, cy: f32, r: f32, color: Color, width: f32) {
        self.set_brush(color);
        let e = D2D1_ELLIPSE { point: D2D_POINT_2F { x: self.dev(cx), y: self.dev(cy) }, radiusX: self.dev(r), radiusY: self.dev(r) };
        unsafe { self.rt.DrawEllipse(&e, self.brush, self.dev(width).max(1.0), None) };
    }

    pub fn fill_circle(&self, cx: f32, cy: f32, r: f32, color: Color) {
        self.set_brush(color);
        let e = D2D1_ELLIPSE { point: D2D_POINT_2F { x: self.dev(cx), y: self.dev(cy) }, radiusX: self.dev(r), radiusY: self.dev(r) };
        unsafe { self.rt.FillEllipse(&e, self.brush) };
    }

    /// 绘制一个内置矢量图标，居中于方形区域 `r`，颜色 `color`。
    pub fn draw_glyph(&self, icon: Icon, r: Rect, color: Color) {
        // 单位盒 [0,1]^2 → r 的映射。
        let m = |u: f32, v: f32| (r.x + u * r.w, r.y + v * r.h);
        let sw = (r.w / 16.0 * 1.3).max(1.0); // 16px 基准下约 1.3px 线宽
        match icon {
            Icon::ChevronDown => {
                let (a, b, c) = (m(0.30, 0.42), m(0.50, 0.62), m(0.70, 0.42));
                self.stroke_polyline(&[a, b, c], color, sw);
            }
            Icon::Hamburger => {
                for v in [0.32, 0.50, 0.68] {
                    let (a, b) = (m(0.20, v), m(0.80, v));
                    self.stroke_polyline(&[a, b], color, sw);
                }
            }
            Icon::Home => {
                let roof = [m(0.18, 0.52), m(0.50, 0.24), m(0.82, 0.52)];
                self.stroke_polyline(&roof, color, sw);
                let body = [m(0.28, 0.48), m(0.28, 0.80), m(0.72, 0.80), m(0.72, 0.48)];
                self.stroke_polyline(&body, color, sw);
            }
            Icon::Folder => {
                let f = [
                    m(0.16, 0.40), m(0.16, 0.74), m(0.84, 0.74), m(0.84, 0.44),
                    m(0.48, 0.44), m(0.40, 0.34), m(0.18, 0.34), m(0.16, 0.40),
                ];
                self.stroke_polygon(&f, color, sw);
            }
            Icon::Star => {
                let cx = r.x + r.w * 0.5;
                let cy = r.y + r.h * 0.52;
                let (ro, ri) = (r.w * 0.40, r.w * 0.17);
                let mut pts = Vec::with_capacity(10);
                for k in 0..10 {
                    let ang = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::PI / 5.0;
                    let rad = if k % 2 == 0 { ro } else { ri };
                    pts.push((cx + rad * ang.cos(), cy + rad * ang.sin()));
                }
                self.fill_polygon(&pts, color);
            }
            Icon::Settings => {
                // 齿轮：8 齿外缘多边形描边 + 中心圆。
                let cx = r.x + r.w * 0.5;
                let cy = r.y + r.h * 0.5;
                let (ro, ri) = (r.w * 0.40, r.w * 0.30);
                let mut pts = Vec::with_capacity(16);
                for k in 0..16 {
                    let ang = k as f32 * std::f32::consts::PI / 8.0;
                    let rad = if k % 2 == 0 { ro } else { ri };
                    pts.push((cx + rad * ang.cos(), cy + rad * ang.sin()));
                }
                self.stroke_polygon(&pts, color, sw);
                self.stroke_circle(cx, cy, r.w * 0.13, color, sw);
            }
            Icon::Info => {
                let cx = r.x + r.w * 0.5;
                let cy = r.y + r.h * 0.5;
                self.stroke_circle(cx, cy, r.w * 0.40, color, sw);
                self.fill_circle(cx, r.y + r.h * 0.32, r.w * 0.045, color);
                let (a, b) = (m(0.50, 0.44), m(0.50, 0.70));
                self.stroke_polyline(&[a, b], color, sw);
            }
            Icon::Success => {
                let cx = r.x + r.w * 0.5;
                let cy = r.y + r.h * 0.5;
                self.stroke_circle(cx, cy, r.w * 0.40, color, sw);
                let chk = [m(0.32, 0.52), m(0.44, 0.64), m(0.68, 0.38)];
                self.stroke_polyline(&chk, color, sw);
            }
            Icon::Warning => {
                let tri = [m(0.50, 0.18), m(0.86, 0.80), m(0.14, 0.80)];
                self.stroke_polygon(&tri, color, sw);
                let (a, b) = (m(0.50, 0.42), m(0.50, 0.60));
                self.stroke_polyline(&[a, b], color, sw);
                self.fill_circle(r.x + r.w * 0.5, r.y + r.h * 0.70, r.w * 0.045, color);
            }
            Icon::Error => {
                let cx = r.x + r.w * 0.5;
                let cy = r.y + r.h * 0.5;
                self.stroke_circle(cx, cy, r.w * 0.40, color, sw);
                let (a, b) = (m(0.38, 0.38), m(0.62, 0.62));
                let (c, d) = (m(0.62, 0.38), m(0.38, 0.62));
                self.stroke_polyline(&[a, b], color, sw);
                self.stroke_polyline(&[c, d], color, sw);
            }
        }
    }

    /// 压入一个裁剪矩形（用于弹出层/列表内容）。需配对 [`Painter::pop_clip`]。
    pub fn push_clip(&self, r: Rect) {
        unsafe {
            self.rt.PushAxisAlignedClip(&self.dev_rect(r), D2D1_ANTIALIAS_MODE_ALIASED);
        }
    }

    pub fn pop_clip(&self) {
        unsafe { self.rt.PopAxisAlignedClip() };
    }

    /// 设置整体世界变换（设备像素空间），用于动画位移等。传 None 复位。
    pub fn set_transform(&self, m: Option<Matrix3x2>) {
        unsafe { self.rt.SetTransform(&m.unwrap_or(Matrix3x2::identity())) };
    }

    /// 命中点是否落在某逻辑矩形内（便捷封装）。
    pub fn hit(&self, r: Rect, p: Point) -> bool {
        r.contains(p)
    }

    /// 结束一帧。返回是否需要重建设备资源（D2DERR_RECREATE_TARGET）。
    pub fn end(self) -> Result<bool> {
        let hr = unsafe { self.rt.EndDraw(None, None) };
        match hr {
            Ok(()) => Ok(false),
            Err(e) if e.code() == windows::core::HRESULT(0x8899_000Cu32 as i32) => Ok(true),
            Err(e) => Err(e),
        }
    }
}

/// Win32 `RECT` → 设备像素宽高。
pub fn rect_size(rc: &RECT) -> (u32, u32) {
    ((rc.right - rc.left).max(0) as u32, (rc.bottom - rc.top).max(0) as u32)
}

// 让上面用到的 Interface trait 被引用（D2DERR_RECREATE_TARGET 经由 .code() 比较）。
const _: fn() = || {
    fn _assert_interface<T: Interface>() {}
    _assert_interface::<ID2D1Factory>();
};
