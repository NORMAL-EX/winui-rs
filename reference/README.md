# reference/

把**真实 WinUI 3 Gallery** 的控件状态截图放在此目录，作为像素级比对的 ground truth。

采集要求（规格第 8 节）：
- 微软商店安装 **WinUI 3 Gallery**（或开源 `microsoft/WinUI-Gallery` 自行编译）。
- 分别在 **深色 / 浅色**、**默认蓝强调色**、**100% 缩放**下打开。
- 对每个控件每个状态（Rest / Hover / Pressed / Disabled / Focused / On-Off 等）截图。
- 命名约定：`<control>_<theme>_<state>.png`，例如 `button_dark_rest.png`。

比对：本项目 `gallery.exe` 用相同窗口尺寸/DPI/主题渲染并截图到 `shots/`，
再用 `python tools/diff.py reference/button_dark_rest.png shots/button_dark_rest.png` 输出 diff。
