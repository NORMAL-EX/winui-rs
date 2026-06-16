//! 下载器（从原项目 src/downloader.rs 迁移；async 流式改阻塞流式，进度用共享状态回传）。
#![allow(dead_code)]

use anyhow::Result;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Default)]
pub struct DownloadProgress {
    pub current: u64,
    pub total: u64,
    pub speed: f64, // MB/s
    pub done: bool,
    pub error: Option<String>,
}

pub type Progress = Arc<Mutex<DownloadProgress>>;

/// 阻塞流式下载（在后台线程内调用），实时写共享进度。
pub fn download(url: &str, path: PathBuf, progress: Progress) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()?;
    let mut resp = client.get(url).send()?;
    let total = resp.content_length().unwrap_or(0);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    {
        let mut p = progress.lock().unwrap();
        p.total = total;
        p.current = 0;
    }
    let mut file = std::fs::File::create(&path)?;
    let mut buf = [0u8; 64 * 1024];
    let mut downloaded = 0u64;
    let start = Instant::now();
    loop {
        let n = resp.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;
        let elapsed = start.elapsed().as_secs_f64();
        let speed = if elapsed > 0.0 { (downloaded as f64 / elapsed) / (1024.0 * 1024.0) } else { 0.0 };
        let mut p = progress.lock().unwrap();
        p.current = downloaded;
        p.speed = speed;
    }
    progress.lock().unwrap().done = true;
    Ok(())
}

/// 在后台线程启动一次下载，返回共享进度句柄。
pub fn spawn_download(url: String, path: PathBuf) -> Progress {
    let progress: Progress = Arc::new(Mutex::new(DownloadProgress::default()));
    let p2 = progress.clone();
    std::thread::spawn(move || {
        if let Err(e) = download(&url, path, p2.clone()) {
            let mut g = p2.lock().unwrap();
            g.error = Some(e.to_string());
            g.done = true;
        }
    });
    progress
}
