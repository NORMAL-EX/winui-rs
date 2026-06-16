#!/usr/bin/env python3
"""逐像素比对脚本（规格第 8 节）。

用法:
    python tools/diff.py reference/button_dark.png shots/button_dark.png [out_diff.png]

输出:
    * 不一致像素占比
    * 最大单通道色差 / 平均色差
    * 差异放大图（红色标注超阈值像素）

注意: 真实 diff 数值需在 Windows 上分别截取「真实 WinUI 3 Gallery」与本项目
gallery 的同尺寸/同 DPI/同主题截图后喂入。本仓库的开发机为 Linux，无法运行
Windows GUI，故 diff 数值由使用者在 Windows 侧产出（见 README 验证章节）。
"""
import sys
from PIL import Image  # pip install pillow


# 抗锯齿边缘允许的灰阶容差（规格 B 档：≤2 灰阶）。
AA_TOLERANCE = 2


def load_rgb(path):
    return Image.open(path).convert("RGB")


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        sys.exit(1)
    ref = load_rgb(sys.argv[1])
    got = load_rgb(sys.argv[2])
    out = sys.argv[3] if len(sys.argv) > 3 else "diff.png"

    if ref.size != got.size:
        print(f"[警告] 尺寸不一致 ref={ref.size} got={got.size}，请用相同窗口尺寸/DPI 截图。")
        # 取交集尺寸继续比对
        w = min(ref.size[0], got.size[0])
        h = min(ref.size[1], got.size[1])
        ref = ref.crop((0, 0, w, h))
        got = got.crop((0, 0, w, h))

    rp = ref.load()
    gp = got.load()
    w, h = ref.size
    diff = Image.new("RGB", (w, h))
    dp = diff.load()

    total = w * h
    mismatched = 0
    over_tol = 0
    max_delta = 0
    sum_delta = 0

    for y in range(h):
        for x in range(w):
            r1, g1, b1 = rp[x, y]
            r2, g2, b2 = gp[x, y]
            d = max(abs(r1 - r2), abs(g1 - g2), abs(b1 - b2))
            sum_delta += d
            if d > max_delta:
                max_delta = d
            if d != 0:
                mismatched += 1
            if d > AA_TOLERANCE:
                over_tol += 1
                dp[x, y] = (255, 0, 0)
            else:
                # 灰度展示原图，便于定位
                gray = (r1 + g1 + b1) // 3
                dp[x, y] = (gray, gray, gray)

    diff.save(out)
    print(f"图像尺寸           : {w}x{h} ({total} px)")
    print(f"不一致像素(>0)     : {mismatched} ({100.0*mismatched/total:.3f}%)")
    print(f"超容差像素(>{AA_TOLERANCE})    : {over_tol} ({100.0*over_tol/total:.3f}%)")
    print(f"最大单通道色差     : {max_delta}")
    print(f"平均单通道色差     : {sum_delta/total:.4f}")
    print(f"差异图已写出       : {out}")
    # B 档判定：超容差像素占比为 0 视为逐像素零差异。
    if over_tol == 0:
        print("结果: ✅ B 档通过（逐像素零差异，AA 容差内）")
    else:
        print("结果: ❌ 存在超容差差异，请回查 token/几何/对齐项")


if __name__ == "__main__":
    main()
