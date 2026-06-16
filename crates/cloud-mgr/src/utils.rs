//! 启动盘扫描（从原项目 src/utils.rs 迁移，纯 fs 逻辑）。
#![allow(dead_code)]

use crate::mode::PluginMode;
use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct BootDrive {
    pub letter: String,
    pub version: String,
}

pub struct BootDriveManager {
    boot_drives: Vec<BootDrive>,
    current_drive: Option<String>,
    mode: PluginMode,
}

impl BootDriveManager {
    pub fn new(mode: PluginMode) -> Self {
        let mut m = Self { boot_drives: Vec::new(), current_drive: None, mode };
        m.boot_drives = m.scan_boot_drives();
        m
    }

    pub fn scan_boot_drives(&self) -> Vec<BootDrive> {
        let mut drives = Vec::new();
        for letter in b'A'..=b'Z' {
            let d = format!("{}:", letter as char);
            match self.mode {
                PluginMode::CloudPE => {
                    if Path::new(&format!("{}\\cloud-pe\\config.json", d)).exists()
                        && Path::new(&format!("{}\\Cloud-PE.iso", d)).exists()
                    {
                        if let Ok(v) = self.read_cloudpe_version(&d) {
                            drives.push(BootDrive { letter: d, version: v });
                        }
                    }
                }
                PluginMode::HotPE => {
                    if Path::new(&format!("{}\\HotPEModule", d)).exists() {
                        drives.push(BootDrive { letter: d, version: "HotPE".into() });
                    } else if Path::new(&format!("{}\\cloud-pe\\config.json", d)).exists()
                        && Path::new(&format!("{}\\Cloud-PE.iso", d)).exists()
                    {
                        drives.push(BootDrive { letter: d, version: "Cloud-PE (HotPE兼容)".into() });
                    }
                }
                PluginMode::Edgeless => {
                    if Path::new(&format!("{}\\Edgeless\\Resource", d)).exists() {
                        drives.push(BootDrive { letter: d, version: "Edgeless".into() });
                    } else if Path::new(&format!("{}\\cloud-pe\\config.json", d)).exists()
                        && Path::new(&format!("{}\\Cloud-PE.iso", d)).exists()
                    {
                        drives.push(BootDrive { letter: d, version: "Cloud-PE (Edgeless兼容)".into() });
                    }
                }
                _ => {}
            }
        }
        drives
    }

    fn read_cloudpe_version(&self, drive: &str) -> Result<String> {
        let content = std::fs::read_to_string(format!("{}\\cloud-pe\\config.json", drive))?;
        let json: serde_json::Value = serde_json::from_str(&content)?;
        json.get("pe")
            .and_then(|pe| pe.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("无法读取版本信息"))
    }

    pub fn get_all_drives(&self) -> Vec<BootDrive> {
        self.boot_drives.clone()
    }
    pub fn get_current_drive(&self) -> Option<String> {
        self.current_drive.clone()
    }
    pub fn set_current_drive(&mut self, drive: String) {
        self.current_drive = Some(drive);
    }
    pub fn reload(&mut self) {
        self.boot_drives = self.scan_boot_drives();
    }
}
