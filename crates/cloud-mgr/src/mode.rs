//! 插件源模式（从原 egui 项目 src/mode.rs 迁移，纯逻辑，未改）。
#![allow(dead_code)] // 部分方法在 phase 2（联网/下载）接入时使用

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginMode {
    CloudPE,
    HotPE,
    Edgeless,
    Select,
}

impl PluginMode {
    pub fn get_api_url(&self) -> &str {
        match self {
            PluginMode::CloudPE => "https://api.cloud-pe.cn/v2/all-plugins.json",
            PluginMode::HotPE => "https://api.hotpe.top/API/HotPE/GetHPMList/",
            PluginMode::Edgeless => "https://api.cloud-pe.cn/v2/all-plugins.json",
            _ => "",
        }
    }

    pub fn get_plugin_folder(&self) -> &str {
        match self {
            PluginMode::CloudPE => "ce-apps",
            PluginMode::HotPE => "HotPEModule",
            PluginMode::Edgeless => "Edgeless\\Resource",
            _ => "",
        }
    }

    pub fn get_enabled_extension(&self) -> &str {
        match self {
            PluginMode::CloudPE => "ce",
            PluginMode::HotPE => "HPM",
            PluginMode::Edgeless => "7z",
            _ => "",
        }
    }

    pub fn get_plugin_market_name(&self) -> &str {
        match self {
            PluginMode::HotPE => "模块市场",
            _ => "插件市场",
        }
    }

    pub fn get_plugin_manage_name(&self) -> &str {
        match self {
            PluginMode::HotPE => "模块管理",
            _ => "插件管理",
        }
    }

    pub fn get_title(&self) -> &str {
        match self {
            PluginMode::CloudPE => "Cloud-PE 插件市场",
            PluginMode::HotPE => "HotPE 模块下载",
            PluginMode::Edgeless => "Edgeless 插件下载",
            _ => "选择插件源",
        }
    }

    pub fn get_server_name(&self) -> &str {
        match self {
            PluginMode::CloudPE => "Cloud-PE",
            PluginMode::HotPE => "HotPE",
            PluginMode::Edgeless => "Edgeless",
            _ => "",
        }
    }
}
