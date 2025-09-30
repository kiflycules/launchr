use anyhow::Result;
use std::process::Command;

use crate::config::{BookmarkConfig, Config};

#[derive(Debug, Clone)]
pub struct Bookmark {
    pub name: String,
    pub path: String,
    pub bookmark_type: String,
}

impl From<BookmarkConfig> for Bookmark {
    fn from(config: BookmarkConfig) -> Self {
        Self { name: config.name, path: config.path, bookmark_type: config.bookmark_type }
    }
}

pub struct BookmarksModule {
    pub bookmarks: Vec<Bookmark>,
    config: Config,
}

impl BookmarksModule {
    pub fn new(config: &Config) -> Self {
        let bookmarks = config.bookmarks.iter().map(|b| b.clone().into()).collect();
        Self { bookmarks, config: config.clone() }
    }

    pub fn open_bookmark(&self, index: usize) -> Result<()> {
        if index >= self.bookmarks.len() { return Ok(()); }
        let bookmark = &self.bookmarks[index];
        match bookmark.bookmark_type.as_str() {
            "url" => self.open_url(&bookmark.path)?,
            "directory" => self.open_directory(&bookmark.path)?,
            "file" => self.open_file(&bookmark.path)?,
            _ => self.open_file(&bookmark.path)?,
        }
        Ok(())
    }

    fn open_url(&self, url: &str) -> Result<()> {
        #[cfg(target_os = "macos")]
        { Command::new("open").arg(url).spawn()?; }
        #[cfg(target_os = "linux")]
        { Command::new("xdg-open").arg(url).spawn()?; }
        #[cfg(target_os = "windows")]
        { Command::new("cmd").args(&["/C", "start", url]).spawn()?; }
        Ok(())
    }

    fn open_directory(&self, path: &str) -> Result<()> {
        #[cfg(target_os = "macos")]
        { Command::new("open").arg(path).spawn()?; }
        #[cfg(target_os = "linux")]
        { Command::new("xdg-open").arg(path).spawn()?; }
        #[cfg(target_os = "windows")]
        { Command::new("explorer").arg(path).spawn()?; }
        Ok(())
    }

    fn open_file(&self, path: &str) -> Result<()> {
        #[cfg(target_os = "macos")]
        { Command::new("open").arg(path).spawn()?; }
        #[cfg(target_os = "linux")]
        { Command::new("xdg-open").arg(path).spawn()?; }
        #[cfg(target_os = "windows")]
        { Command::new("cmd").args(&["/C", "start", "", path]).spawn()?; }
        Ok(())
    }

    pub fn add_from_string(&mut self, input: &str) -> Result<()> {
        let parts: Vec<&str> = input.split('|').collect();
        if parts.len() < 2 { anyhow::bail!("Invalid format. Use: name|path|type"); }
        let name = parts[0].trim().to_string();
        let path = parts[1].trim().to_string();
        let bookmark_type = if parts.len() > 2 { parts[2].trim().to_string() } else { "file".to_string() };
        let bookmark = Bookmark { name: name.clone(), path: path.clone(), bookmark_type: bookmark_type.clone() };
        let config_bookmark = BookmarkConfig { name, path, bookmark_type };
        self.bookmarks.push(bookmark);
        self.config.add_bookmark(config_bookmark)?;
        Ok(())
    }

    pub fn delete(&mut self, index: usize) {
        if index < self.bookmarks.len() {
            self.bookmarks.remove(index);
            let _ = self.config.remove_bookmark(index);
        }
    }
}


