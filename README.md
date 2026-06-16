# winui-rs / fluentpx

用纯 **Rust + Direct2D** 像素级（1:1）复刻 **WinUI 3 Fluent** 控件库，**不依赖**
WinUI3 / Windows App SDK / .NET，产物体积控制在几 MB。

- `crates/fluentpx`：组件库（可被依赖）。颜色 token、圆角、内边距、渐变、状态切换、
  动画曲线等数值**全部取自官方 `microsoft-ui-xaml` 源码真值**，逐项在源码注释中标注出处。
- `crates/gallery`：展示 exe，Win32 窗口 + DWM（深色标题栏 / Mica / 圆角）+ Per-Monitor v2
  高 DPI + 深浅主题切换 + 动画驱动，陈列各控件的各状态。

## 技术栈

| 方面 | 选型 |
|---|---|
| 语言 / 绑定 | Rust + `windows` crate（pin `=0.58.0`） |
| 2D 渲染 | Direct2D（`ID2D1HwndRenderTarget`） |
| 文字 | DirectWrite，`Segoe UI Variable Text`（回退 `Segoe UI`） |
| 窗口效果 | DWM：Mica 背景 / 圆角 / 深色标题栏 |
| 窗口 / 消息 | Win32 `CreateWindowExW` + `WNDPROC` |

不引入任何 GUI 框架（egui/slint/iced 等），全部自绘。

## 控件进度

| 控件 | 状态 |
|---|---|
| Button（普通） | ✅ 已实现（rest/hover/pressed/disabled + 渐变边框 + 83ms 背景过渡） |
| AccentButton（蓝） | ✅ 已实现（OuterBorderEdge + OnAccent 渐变边框 + 各状态） |
| ToggleSwitch | ✅ 已实现（knob 位移 + 配色交叉淡入 + 缓动） |
| Slider | ✅ 已实现（track/填充/thumb 三段 + thumb 形变动画 + 拖动） |
| ComboBox | ⏳ 进行中 |
| ListView/ListBox | ⏳ 进行中 |
| TabView | ⏳ 进行中 |
| ToolTip | ⏳ 进行中 |
| ContentDialog | ⏳ 进行中 |

## 如何拿到编译好的 exe（下载链接）

下载链接只能在你自己的 GitHub 仓库里由 Actions 生成。本仓库已配置
`.github/workflows/build.yml`（runner = `windows-latest`）：

1. **每次 push** → 进入仓库 **Actions** 标签 → 打开该次 run → 在 **Artifacts** 区下载
   `gallery-exe`（内含 `gallery.exe`）。
2. **打 tag 发版**（稳定链接）：
   ```bash
   git tag v0.1 && git push --tags
   ```
   Actions 跑完后到 **Releases** 页下载 `gallery.exe`。

## 本地构建

需在 **Windows** 上：
```bash
cargo build --release -p gallery
# 产物: target/release/gallery.exe
./target/release/gallery.exe
```

## 验证 1:1（规格第 8 节）

1. 安装真实 **WinUI 3 Gallery**，在深色/浅色、默认蓝强调、100% 缩放下对每个控件每个状态截图，
   存入 `reference/`（命名见 `reference/README.md`）。
2. 用本项目 `gallery.exe` 同尺寸/同 DPI/同主题渲染、截图到 `shots/`。
3. 逐像素 diff：
   ```bash
   python tools/diff.py reference/button_dark_rest.png shots/button_dark_rest.png
   ```
   输出不一致像素占比、最大/平均色差、差异图。

## 关于开发环境的诚实说明

本仓库的代码在 **Linux** 开发机上编写，使用 `cargo check --target x86_64-pc-windows-gnu`
做**编译期校验**。但 Direct2D/DWrite/DWM 是 Windows 专有 API，**Linux 机器无法链接运行
GUI，也无法运行微软的 WinUI 3 Gallery**。因此：

- 代码的**编译正确性**由 Windows 目标的 `cargo check` 与 CI（`windows-latest`）保证。
- **运行与逐像素 diff** 必须在 Windows 上完成（下载 CI 产出的 exe，或本地编译）。
- 含实时模糊的弹出层 Acrylic / 阴影若无法做到逐像素零差异，会在对应控件处明确标注差距与原因
  （DirectComposition backdrop 较难，先用 `SolidBackgroundFillColor*` 近似，**标注为待办**）。

## 已知限制（规格第 11 节）

- **可编辑 ComboBox / 中文输入**需对接 TSF，本批先做不可编辑 ComboBox，代码中标注。
- **无障碍 (UIA)**：`widget::Widget` 已预留 `accessible_role/name` 接口，实装暂缓。
- **弹出层实时 Acrylic / 阴影**：先用纯色近似，标注「待办：实时模糊」。

## 许可

MIT。`microsoft-ui-xaml` 亦为 MIT，仅作为数值真值来源参考。
