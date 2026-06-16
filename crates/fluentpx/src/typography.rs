//! 字体排印：WinUI 字号阶梯（type ramp）+ DirectWrite `IDWriteTextFormat`。
//!
//! 控件内容默认字号 `ControlContentThemeFontSize = 14`（Body）。
//! 字体族用可变字体 `Segoe UI Variable Text`（光学尺寸 Text 轴），回退 `Segoe UI`。

use windows::core::{Result, HSTRING};
use windows::Win32::Graphics::DirectWrite::*;

/// 主字体族（可变字体的 Text 光学尺寸变体）。DirectWrite 不存在时自动回退到 `Segoe UI`。
pub const FONT_FAMILY: &str = "Segoe UI Variable Text";
pub const FONT_FAMILY_FALLBACK: &str = "Segoe UI";

/// 内容控件默认字号 `ControlContentThemeFontSize`。
pub const CONTROL_CONTENT_FONT_SIZE: f32 = 14.0;

/// WinUI type ramp 的一档：字号 + 字重 + 行高（设备无关像素）。
#[derive(Clone, Copy)]
pub struct TextStyle {
    pub size: f32,
    pub weight: DWRITE_FONT_WEIGHT,
    pub line_height: f32,
}

impl TextStyle {
    pub const CAPTION: TextStyle = TextStyle { size: 12.0, weight: DWRITE_FONT_WEIGHT_NORMAL, line_height: 16.0 };
    pub const BODY: TextStyle = TextStyle { size: 14.0, weight: DWRITE_FONT_WEIGHT_NORMAL, line_height: 20.0 };
    pub const BODY_STRONG: TextStyle = TextStyle { size: 14.0, weight: DWRITE_FONT_WEIGHT_SEMI_BOLD, line_height: 20.0 };
    pub const BODY_LARGE: TextStyle = TextStyle { size: 18.0, weight: DWRITE_FONT_WEIGHT_NORMAL, line_height: 24.0 };
    pub const SUBTITLE: TextStyle = TextStyle { size: 20.0, weight: DWRITE_FONT_WEIGHT_SEMI_BOLD, line_height: 28.0 };
    pub const TITLE: TextStyle = TextStyle { size: 28.0, weight: DWRITE_FONT_WEIGHT_SEMI_BOLD, line_height: 36.0 };
    pub const TITLE_LARGE: TextStyle = TextStyle { size: 40.0, weight: DWRITE_FONT_WEIGHT_SEMI_BOLD, line_height: 52.0 };
    pub const DISPLAY: TextStyle = TextStyle { size: 68.0, weight: DWRITE_FONT_WEIGHT_SEMI_BOLD, line_height: 92.0 };
}

/// 用给定 DWrite 工厂创建一个 `IDWriteTextFormat`。
///
/// `scale` 为 DPI 缩放（dpi/96）：DirectWrite 以设备无关像素工作，但我们把
/// 字号按 scale 预乘，使整个渲染目标统一在设备像素上对齐（见 `dpi`）。
pub fn create_text_format(
    dwrite: &IDWriteFactory,
    style: TextStyle,
    scale: f32,
) -> Result<IDWriteTextFormat> {
    let family = HSTRING::from(FONT_FAMILY);
    let locale = HSTRING::from("en-US");
    unsafe {
        let format = dwrite.CreateTextFormat(
            &family,
            None,
            style.weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            style.size * scale,
            &locale,
        )?;
        // 默认左对齐、垂直居中由布局逻辑控制；这里禁用自动换行以匹配按钮等单行内容。
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        // 用 UNIFORM 行高，按字号比例锁定，避免不同字体回退导致行盒抖动。
        format.SetLineSpacing(
            DWRITE_LINE_SPACING_METHOD_UNIFORM,
            style.line_height * scale,
            style.line_height * scale * 0.8,
        )?;
        Ok(format)
    }
}
