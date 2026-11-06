use std::fs;
use std::path::PathBuf;

#[derive(Default, Clone)]
pub struct Config {
    pub target_lang: Option<String>,
    pub last_source_lang: Option<String>,
}

fn dir() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("reverso-linux"));
        }
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/reverso-linux"))
}

fn path() -> Option<PathBuf> {
    dir().map(|d| d.join("config"))
}

pub fn load() -> Config {
    let mut cfg = Config::default();
    let Some(path) = path() else {
        return cfg;
    };
    let Ok(text) = fs::read_to_string(path) else {
        return cfg;
    };
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("target_lang=") {
            cfg.target_lang = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("last_source_lang=") {
            cfg.last_source_lang = Some(v.trim().to_string());
        }
    }
    cfg
}

pub fn save(target: Option<&str>, source: Option<&str>) {
    let Some(dir) = dir() else {
        return;
    };
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("config");
    let mut out = String::new();
    if let Some(t) = target {
        out.push_str("target_lang=");
        out.push_str(t);
        out.push('\n');
    }
    if let Some(s) = source {
        out.push_str("last_source_lang=");
        out.push_str(s);
        out.push('\n');
    }
    let _ = fs::write(path, out);
}

pub fn save_target(lang: &str) {
    let cfg = load();
    save(Some(lang), cfg.last_source_lang.as_deref());
}
