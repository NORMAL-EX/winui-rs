//! Cloud-MGR：Cloud-PE 插件市场/管理器，从 egui 迁移到 fluentpx（纯 Rust + Direct2D）。
//! 本版完成应用外壳 + 导航 + 可用设置页；联网/下载/写盘见 app.rs 中 phase 2 标注。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod downloader;
mod mode;
mod plugins;
mod utils;

use fluentpx::host::{run, WindowOptions};
use mode::PluginMode;

fn main() {
    // 解析命令行模式（与原项目一致：--hpm / --edgeless，默认 CloudPE）。
    let args: Vec<String> = std::env::args().collect();
    let mode = match args.get(1).map(|s| s.as_str()) {
        Some("--hpm") => PluginMode::HotPE,
        Some("--edgeless") => PluginMode::Edgeless,
        _ => PluginMode::CloudPE,
    };

    let opts = WindowOptions {
        title: mode.get_title().to_string(),
        width: 1024,
        height: 630,
        min_width: 820,
        min_height: 560,
    };

    let _ = run(opts, move |theme| Box::new(app::CloudMgrApp::new(theme, mode)));
}
