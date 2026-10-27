// Path variables ($APPDATA/...) and the file-access permission check behind tauri.io.
use std::{
    path::{Component, Path, PathBuf},
    sync::Mutex,
};

use tauri::{AppHandle, Manager};

use super::config;

#[derive(Clone, Copy)]
pub enum Access {
    Read,
    Write,
}

const DEFAULT_READ: &[&str] = &["$APPDATA", "$APPCONFIG", "$APPCACHE", "$APPLOG", "$TEMP", "$RESOURCE"];
const DEFAULT_WRITE: &[&str] = &["$APPDATA", "$APPCONFIG", "$APPCACHE", "$APPLOG", "$TEMP"];

/// Paths the user picked in a dialog or dropped on a window this session.
static GRANTS: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

pub fn grant(path: &Path) {
    GRANTS.lock().unwrap().push(resolve_real(path));
}

pub fn var_dir(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let p = app.path();
    let r = match name {
        "APPDATA" => p.app_data_dir(),
        "APPCONFIG" => p.app_config_dir(),
        "APPCACHE" => p.app_cache_dir(),
        "APPLOG" => p.app_log_dir(),
        "APPLOCALDATA" => p.app_local_data_dir(),
        "RESOURCE" => p.resource_dir(),
        "TEMP" => p.temp_dir(),
        "HOME" => p.home_dir(),
        "DOCUMENT" => p.document_dir(),
        "DOWNLOAD" => p.download_dir(),
        "DESKTOP" => p.desktop_dir(),
        "PICTURE" => p.picture_dir(),
        "AUDIO" => p.audio_dir(),
        "VIDEO" => p.video_dir(),
        _ => return Err(format!("unknown path variable ${name}")),
    };
    r.map_err(|e| e.to_string())
}

/// "$APPDATA/notes.txt" -> real path. Relative paths live under $APPDATA.
pub fn expand(app: &AppHandle, raw: &str) -> Result<PathBuf, String> {
    if let Some(rest) = raw.strip_prefix('$') {
        let end = rest.find(|c| c == '/' || c == '\\').unwrap_or(rest.len());
        let (var, tail) = rest.split_at(end);
        let tail = tail.trim_start_matches(|c| c == '/' || c == '\\');
        return Ok(var_dir(app, var)?.join(tail));
    }
    let p = PathBuf::from(raw);
    if p.is_absolute() {
        Ok(p)
    } else {
        Ok(var_dir(app, "APPDATA")?.join(p))
    }
}

fn lexical(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Resolve ".." and symlinks (for the part that exists) so scope checks can't be dodged.
fn resolve_real(p: &Path) -> PathBuf {
    let p = lexical(p);
    let mut existing = p.clone();
    let mut rest = Vec::new();
    while !existing.exists() {
        match existing.file_name() {
            Some(n) => {
                rest.push(n.to_owned());
                existing.pop();
            }
            None => break,
        }
    }
    let mut real = existing.canonicalize().unwrap_or(existing);
    for n in rest.into_iter().rev() {
        real.push(n);
    }
    real
}

/// Returns the resolved path if access is allowed by mare.json (or a user grant).
pub fn check(app: &AppHandle, raw: &str, access: Access) -> Result<PathBuf, String> {
    let path = expand(app, raw)?;
    let real = resolve_real(&path);

    let (key, defaults) = match access {
        Access::Read => ("read", DEFAULT_READ),
        Access::Write => ("write", DEFAULT_WRITE),
    };
    for root in config::list_or(&["permissions", "fs", key], defaults) {
        if let Ok(r) = expand(app, &root) {
            if real.starts_with(resolve_real(&r)) {
                return Ok(real);
            }
        }
    }
    if GRANTS.lock().unwrap().iter().any(|g| real.starts_with(g)) {
        return Ok(real);
    }
    Err(format!(
        "mare: {key} access to '{raw}' is not allowed. Add a folder to permissions.fs.{key} in mare.json, or let the user pick it with tauri.dialog."
    ))
}
