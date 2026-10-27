// tauri.io: file access, permission-checked (see permissions.fs in mare.json).
use std::{fs, io::Write as _, path::PathBuf, time::UNIX_EPOCH};

use mlua::{Lua, Table};
use tauri::AppHandle;

use super::{
    paths::{self, Access},
    rt_err,
};

fn guard(app: &AppHandle, raw: &str, access: Access) -> mlua::Result<PathBuf> {
    paths::check(app, raw, access).map_err(rt_err)
}

fn write_file(app: &AppHandle, path: &str, data: &[u8], append: bool) -> mlua::Result<()> {
    let p = guard(app, path, Access::Write)?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(rt_err)?;
    }
    let mut f = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(append)
        .truncate(!append)
        .open(&p)
        .map_err(rt_err)?;
    f.write_all(data).map_err(rt_err)
}

pub fn install(app: &AppHandle, lua: &Lua, tauri: &Table) -> mlua::Result<()> {
    let t = lua.create_table()?;

    {
        let a = app.clone();
        t.set(
            "read_text",
            lua.create_function(move |_, path: String| {
                fs::read_to_string(guard(&a, &path, Access::Read)?).map_err(rt_err)
            })?,
        )?;
    }
    {
        let a = app.clone();
        t.set(
            "read_bytes",
            lua.create_function(move |lua, path: String| {
                let bytes = fs::read(guard(&a, &path, Access::Read)?).map_err(rt_err)?;
                lua.create_string(&bytes)
            })?,
        )?;
    }
    for (name, append) in [("write_text", false), ("write_bytes", false), ("append_text", true)] {
        let a = app.clone();
        t.set(
            name,
            lua.create_function(move |_, (path, data): (String, mlua::String)| {
                write_file(&a, &path, &data.as_bytes(), append)
            })?,
        )?;
    }
    {
        let a = app.clone();
        t.set(
            "exists",
            lua.create_function(move |_, path: String| Ok(guard(&a, &path, Access::Read)?.exists()))?,
        )?;
    }
    {
        let a = app.clone();
        t.set(
            "stat",
            lua.create_function(move |lua, path: String| {
                let md = fs::metadata(guard(&a, &path, Access::Read)?).map_err(rt_err)?;
                let row = lua.create_table()?;
                row.set("size", md.len())?;
                row.set("is_dir", md.is_dir())?;
                row.set("is_file", md.is_file())?;
                let modified = md
                    .modified()
                    .ok()
                    .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs());
                row.set("modified", modified)?;
                Ok(row)
            })?,
        )?;
    }
    {
        let a = app.clone();
        t.set(
            "list_dir",
            lua.create_function(move |lua, path: String| {
                let mut rows = Vec::new();
                for entry in fs::read_dir(guard(&a, &path, Access::Read)?).map_err(rt_err)? {
                    let entry = entry.map_err(rt_err)?;
                    let ft = entry.file_type().map_err(rt_err)?;
                    let row = lua.create_table()?;
                    row.set("name", entry.file_name().to_string_lossy().to_string())?;
                    row.set("is_dir", ft.is_dir())?;
                    row.set("is_file", ft.is_file())?;
                    rows.push(row);
                }
                Ok(rows)
            })?,
        )?;
    }
    {
        let a = app.clone();
        t.set(
            "mkdir",
            lua.create_function(move |_, (path, recursive): (String, Option<bool>)| {
                let p = guard(&a, &path, Access::Write)?;
                if recursive.unwrap_or(true) {
                    fs::create_dir_all(p).map_err(rt_err)
                } else {
                    fs::create_dir(p).map_err(rt_err)
                }
            })?,
        )?;
    }
    {
        let a = app.clone();
        t.set(
            "remove",
            lua.create_function(move |_, (path, recursive): (String, Option<bool>)| {
                let p = guard(&a, &path, Access::Write)?;
                if p.is_dir() {
                    if recursive.unwrap_or(false) {
                        fs::remove_dir_all(p).map_err(rt_err)
                    } else {
                        fs::remove_dir(p).map_err(rt_err)
                    }
                } else {
                    fs::remove_file(p).map_err(rt_err)
                }
            })?,
        )?;
    }
    {
        let a = app.clone();
        t.set(
            "rename",
            lua.create_function(move |_, (from, to): (String, String)| {
                let f = guard(&a, &from, Access::Write)?;
                let d = guard(&a, &to, Access::Write)?;
                fs::rename(f, d).map_err(rt_err)
            })?,
        )?;
    }
    {
        let a = app.clone();
        t.set(
            "copy",
            lua.create_function(move |_, (from, to): (String, String)| {
                let f = guard(&a, &from, Access::Read)?;
                let d = guard(&a, &to, Access::Write)?;
                fs::copy(f, d).map(|_| ()).map_err(rt_err)
            })?,
        )?;
    }

    tauri.set("io", t)
}
