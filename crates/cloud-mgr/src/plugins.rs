//! 插件数据模型 + 管理器（从原项目 src/plugins.rs 迁移；fetch 由 async 改阻塞客户端，
//! 本地扫描/启用禁用/卸载/版本比较等逻辑沿用原实现）。
#![allow(dead_code)]

use crate::mode::PluginMode;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    pub name: String,
    pub size: String,
    pub version: String,
    pub author: String,
    #[serde(rename = "description", alias = "describe", default)]
    pub describe: String,
    #[serde(default)]
    pub file: String,
    pub link: String,
}

impl Plugin {
    fn unique_key(&self) -> String {
        format!("{}_{}_{}_{}", self.name, self.version, self.author, self.size)
    }
    pub fn plugin_id(&self) -> String {
        format!("{}_{}", self.name, self.author)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCategory {
    #[serde(rename = "category", alias = "class")]
    pub class: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub list: Vec<Plugin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AllPluginsResponse {
    code: i32,
    message: String,
    data: AllPluginsData,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AllPluginsData {
    #[serde(default)]
    plugins: Vec<PluginCategory>,
    #[serde(default)]
    edgeless_plugins: Vec<PluginCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HotPEResponse {
    state: String,
    data: Vec<HotPECategory>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HotPECategory {
    class: String,
    #[serde(default)]
    icon: Option<String>,
    list: Vec<HotPEPlugin>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HotPEPlugin {
    name: String,
    size: serde_json::Value,
    link: String,
}

/// 阻塞式拉取插件列表（在后台线程内调用）。
pub fn fetch_plugins(mode: PluginMode) -> Result<Vec<PluginCategory>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let text = client.get(mode.get_api_url()).send()?.text()?;

    match mode {
        PluginMode::CloudPE | PluginMode::Edgeless => {
            let resp: AllPluginsResponse = serde_json::from_str(&text)?;
            if resp.code != 200 {
                anyhow::bail!("获取插件列表失败: {}", resp.message);
            }
            let mut categories = match mode {
                PluginMode::Edgeless => resp.data.edgeless_plugins,
                _ => resp.data.plugins,
            };
            for category in &mut categories {
                let mut seen = HashSet::new();
                category.list.retain(|p| seen.insert(p.unique_key()));
            }
            Ok(categories)
        }
        PluginMode::HotPE => {
            let resp: HotPEResponse = serde_json::from_str(&text)?;
            if resp.state != "success" {
                anyhow::bail!("获取 HotPE 模块列表失败");
            }
            let mut categories = Vec::new();
            for cat in resp.data {
                let mut plugins = Vec::new();
                for hp in cat.list {
                    let base = hp.name.trim_end_matches(".HPM");
                    let parts: Vec<&str> = base.split('_').collect();
                    let (name, author, version, describe) = if parts.len() >= 4 {
                        (parts[0].into(), parts[1].into(), parts[2].into(), parts[3..].join("_"))
                    } else if parts.len() == 3 {
                        (parts[0].into(), parts[1].into(), parts[2].into(), String::new())
                    } else {
                        (hp.name.clone(), String::new(), String::new(), String::new())
                    };
                    let size = match &hp.size {
                        serde_json::Value::Number(n) => n.as_i64().map(format_file_size).unwrap_or_else(|| "未知大小".into()),
                        serde_json::Value::String(s) => s.clone(),
                        _ => "未知大小".into(),
                    };
                    plugins.push(Plugin { name, size, version, author, describe, file: hp.name, link: hp.link });
                }
                categories.push(PluginCategory { class: cat.class, icon: cat.icon, list: plugins });
            }
            Ok(categories)
        }
        _ => anyhow::bail!("不支持的模式"),
    }
}

pub struct PluginManager {
    pub categories: Vec<PluginCategory>,
    enabled_plugins: Vec<Plugin>,
    disabled_plugins: Vec<Plugin>,
    enabled_plugin_map: HashMap<String, Plugin>,
    mode: PluginMode,
}

impl PluginManager {
    pub fn new(mode: PluginMode) -> Self {
        Self {
            categories: Vec::new(),
            enabled_plugins: Vec::new(),
            disabled_plugins: Vec::new(),
            enabled_plugin_map: HashMap::new(),
            mode,
        }
    }

    pub fn set_categories(&mut self, c: Vec<PluginCategory>) {
        self.categories = c;
    }

    /// 扁平化全部市场插件（去重）。
    pub fn all_market_plugins(&self) -> Vec<Plugin> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for c in &self.categories {
            for p in &c.list {
                if seen.insert(p.unique_key()) {
                    out.push(p.clone());
                }
            }
        }
        out
    }

    pub fn search_plugins(&self, keyword: &str) -> Vec<Plugin> {
        let kw = keyword.to_lowercase();
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for c in &self.categories {
            for p in &c.list {
                let hay = format!("{} {} {} {}", p.name, p.author, p.describe, p.version).to_lowercase();
                if hay.contains(&kw) && seen.insert(p.unique_key()) {
                    out.push(p.clone());
                }
            }
        }
        out
    }

    pub fn load_local_plugins(&mut self, drive: &str) -> Result<()> {
        let dir = format!("{}\\{}", drive, self.mode.get_plugin_folder());
        let dir_path = Path::new(&dir);
        if !dir_path.exists() {
            fs::create_dir_all(dir_path)?;
        }
        self.enabled_plugins.clear();
        self.disabled_plugins.clear();
        self.enabled_plugin_map.clear();
        let (mut se, mut sd) = (HashSet::new(), HashSet::new());
        let en = self.mode.get_enabled_extension().to_lowercase();
        let di = self.mode.get_disabled_extension().to_lowercase();
        for entry in fs::read_dir(dir_path)? {
            let path = entry?.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let fname = path.file_name().unwrap().to_string_lossy().to_string();
            let (is_en, is_di) = match self.mode {
                PluginMode::HotPE => (ext == "hpm" && !fname.ends_with(".hpm.off"), fname.ends_with(".hpm.off")),
                _ => (ext == en, ext == di),
            };
            if !(is_en || is_di) {
                continue;
            }
            if let Some(p) = self.parse_plugin_file(&path) {
                let key = p.unique_key();
                if is_en {
                    if se.insert(key) {
                        self.enabled_plugin_map.insert(p.plugin_id(), p.clone());
                        self.enabled_plugins.push(p);
                    }
                } else if sd.insert(key) {
                    self.disabled_plugins.push(p);
                }
            }
        }
        Ok(())
    }

    fn parse_plugin_file(&self, path: &Path) -> Option<Plugin> {
        let fname = path.file_name()?.to_string_lossy().to_string();
        let size = fs::metadata(path).ok().map(|m| format!("{:.2} MB", m.len() as f64 / 1024.0 / 1024.0)).unwrap_or_default();
        match self.mode {
            PluginMode::CloudPE => {
                let parts: Vec<&str> = fname.split('_').collect();
                if parts.len() >= 4 {
                    let de = parts[3..].join("_");
                    let describe = de.strip_suffix(".ce").or_else(|| de.strip_suffix(".CBK")).unwrap_or(&de).to_string();
                    Some(Plugin { name: parts[0].into(), size, version: parts[1].into(), author: parts[2].into(), describe, file: fname, link: String::new() })
                } else {
                    None
                }
            }
            PluginMode::HotPE => {
                let base = fname.strip_suffix(".HPM").or_else(|| fname.strip_suffix(".hpm.off")).unwrap_or(&fname);
                let parts: Vec<&str> = base.split('_').collect();
                if parts.len() >= 3 {
                    let describe = if parts.len() > 3 { parts[3..].join("_") } else { String::new() };
                    Some(Plugin { name: parts[0].into(), size, version: parts[2].into(), author: parts[1].into(), describe, file: fname, link: String::new() })
                } else {
                    None
                }
            }
            PluginMode::Edgeless => {
                let base = fname.strip_suffix(".7z").or_else(|| fname.strip_suffix(".7zf")).unwrap_or(&fname);
                let parts: Vec<&str> = base.split('_').collect();
                if parts.len() >= 3 {
                    Some(Plugin { name: parts[0].into(), size, version: parts[1].into(), author: parts[2..].join("_"), describe: String::new(), file: fname, link: String::new() })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn enable_plugin(&mut self, drive: &str, fname: &str) -> Result<()> {
        let dir = format!("{}\\{}", drive, self.mode.get_plugin_folder());
        let from = Path::new(&dir).join(fname);
        if !from.exists() {
            anyhow::bail!("文件不存在");
        }
        let to_name = match self.mode {
            PluginMode::CloudPE => fname.replace(".CBK", ".ce"),
            PluginMode::HotPE => fname.replace(".hpm.off", ".HPM"),
            PluginMode::Edgeless => fname.replace(".7zf", ".7z"),
            _ => return Ok(()),
        };
        fs::rename(&from, Path::new(&dir).join(&to_name))?;
        self.load_local_plugins(drive)
    }

    pub fn disable_plugin(&mut self, drive: &str, fname: &str) -> Result<()> {
        let dir = format!("{}\\{}", drive, self.mode.get_plugin_folder());
        let from = Path::new(&dir).join(fname);
        if !from.exists() {
            anyhow::bail!("文件不存在");
        }
        let to_name = match self.mode {
            PluginMode::CloudPE => fname.replace(".ce", ".CBK"),
            PluginMode::HotPE => {
                if fname.ends_with(".HPM") { fname.replace(".HPM", ".hpm.off") } else { format!("{}.off", fname) }
            }
            PluginMode::Edgeless => fname.replace(".7z", ".7zf"),
            _ => return Ok(()),
        };
        fs::rename(&from, Path::new(&dir).join(&to_name))?;
        self.load_local_plugins(drive)
    }

    pub fn delete_plugin_file(&self, drive: &str, fname: &str) -> Result<()> {
        let dir = format!("{}\\{}", drive, self.mode.get_plugin_folder());
        let p = Path::new(&dir).join(fname);
        if !p.exists() {
            anyhow::bail!("文件不存在");
        }
        fs::remove_file(&p)?;
        Ok(())
    }

    pub fn enabled_plugins(&self) -> &Vec<Plugin> {
        &self.enabled_plugins
    }
    pub fn disabled_plugins(&self) -> &Vec<Plugin> {
        &self.disabled_plugins
    }
}

fn format_file_size(size: i64) -> String {
    if size < 1024 {
        format!("{} B", size)
    } else if size < 1024 * 1024 {
        format!("{:.2} KB", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.2} MB", size as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.2} GB", size as f64 / 1024.0 / 1024.0 / 1024.0)
    }
}
