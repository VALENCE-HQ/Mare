// tauri.dialog: native file pickers and message boxes.
// Files the user picks are automatically readable/writable by tauri.io for this session.
use std::path::PathBuf;

use mlua::{Lua, Table, Value};
use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

use crate::mare::{opt, paths, rt_err};

#[derive(Default)]
struct Opts {
    title: Option<String>,
    filters: Vec<(String, Vec<String>)>,
    dir: Option<String>,
    file_name: Option<String>,
    multiple: bool,
    directory: bool,
}

fn parse(opts: Option<Table>) -> Opts {
    let mut o = Opts::default();
    if let Some(t) = opts {
        o.title = opt(&t, "title");
        o.dir = opt(&t, "default_path");
        o.file_name = opt(&t, "default_name");
        o.multiple = opt(&t, "multiple").unwrap_or(false);
        o.directory = opt(&t, "directory").unwrap_or(false);
        if let Some(fs) = opt::<Table>(&t, "filters") {
            for f in fs.sequence_values::<Table>().flatten() {
                let name: String = opt(&f, "name").unwrap_or_default();
                let exts: Vec<String> = opt(&f, "extensions").unwrap_or_default();
                o.filters.push((name, exts));
            }
        }
    }
    o
}

fn kind_of(s: Option<String>) -> MessageDialogKind {
    match s.as_deref() {
        Some("warning") => MessageDialogKind::Warning,
        Some("error") => MessageDialogKind::Error,
        _ => MessageDialogKind::Info,
    }
}

pub fn install(app: &AppHandle, lua: &Lua, tauri: &Table) -> mlua::Result<()> {
    app.plugin(tauri_plugin_dialog::init()).map_err(rt_err)?;
    let t = lua.create_table()?;

    // tauri.dialog.open({ title, filters = {{ name, extensions }}, multiple, directory, default_path })
    //   -> path | {paths} | nil
    {
        let app = app.clone();
        t.set(
            "open",
            lua.create_async_function(move |lua, opts: Option<Table>| {
                let app = app.clone();
                async move {
                    let o = parse(opts);
                    let (multiple, directory) = (o.multiple, o.directory);
                    let picked: Option<Vec<PathBuf>> = tauri::async_runtime::spawn_blocking(move || {
                        let mut d = app.dialog().file();
                        if let Some(t) = &o.title { d = d.set_title(t.clone()); }
                        for (name, exts) in &o.filters {
                            let e: Vec<&str> = exts.iter().map(String::as_str).collect();
                            d = d.add_filter(name.clone(), &e);
                        }
                        if let Some(p) = &o.dir { d = d.set_directory(p); }
                        if o.directory {
                            d.blocking_pick_folder().and_then(|p| p.into_path().ok()).map(|p| vec![p])
                        } else if o.multiple {
                            d.blocking_pick_files()
                                .map(|v| v.into_iter().filter_map(|p| p.into_path().ok()).collect())
                        } else {
                            d.blocking_pick_file().and_then(|p| p.into_path().ok()).map(|p| vec![p])
                        }
                    })
                    .await
                    .map_err(rt_err)?;

                    let Some(list) = picked else { return Ok(Value::Nil) };
                    for p in &list { paths::grant(p); }
                    let strs: Vec<String> = list.iter().map(|p| p.display().to_string()).collect();
                    if multiple && !directory {
                        Ok(Value::Table(lua.create_sequence_from(strs)?))
                    } else {
                        Ok(Value::String(lua.create_string(&strs[0])?))
                    }
                }
            })?,
        )?;
    }

    // tauri.dialog.save({ title, filters, default_path, default_name }) -> path | nil
    {
        let app = app.clone();
        t.set(
            "save",
            lua.create_async_function(move |lua, opts: Option<Table>| {
                let app = app.clone();
                async move {
                    let o = parse(opts);
                    let picked: Option<PathBuf> = tauri::async_runtime::spawn_blocking(move || {
                        let mut d = app.dialog().file();
                        if let Some(t) = &o.title { d = d.set_title(t.clone()); }
                        for (name, exts) in &o.filters {
                            let e: Vec<&str> = exts.iter().map(String::as_str).collect();
                            d = d.add_filter(name.clone(), &e);
                        }
                        if let Some(p) = &o.dir { d = d.set_directory(p); }
                        if let Some(n) = &o.file_name { d = d.set_file_name(n.clone()); }
                        d.blocking_save_file().and_then(|p| p.into_path().ok())
                    })
                    .await
                    .map_err(rt_err)?;
                    match picked {
                        Some(p) => {
                            paths::grant(&p);
                            Ok(Value::String(lua.create_string(p.display().to_string())?))
                        }
                        None => Ok(Value::Nil),
                    }
                }
            })?,
        )?;
    }

    // tauri.dialog.message("text", { title, kind = "info"|"warning"|"error" })
    // tauri.dialog.confirm("text", { title, kind, ok = "Yes", cancel = "No" }) -> boolean
    for (name, confirm) in [("message", false), ("confirm", true)] {
        let app = app.clone();
        t.set(
            name,
            lua.create_async_function(move |_, (text, opts): (String, Option<Table>)| {
                let app = app.clone();
                async move {
                    let title = opts.as_ref().and_then(|o| opt::<String>(o, "title")).unwrap_or_default();
                    let kind = kind_of(opts.as_ref().and_then(|o| opt::<String>(o, "kind")));
                    let ok = opts.as_ref().and_then(|o| opt::<String>(o, "ok"));
                    let cancel = opts.as_ref().and_then(|o| opt::<String>(o, "cancel"));
                    let buttons = match (confirm, ok, cancel) {
                        (false, _, _) => MessageDialogButtons::Ok,
                        (true, Some(o), Some(c)) => MessageDialogButtons::OkCancelCustom(o, c),
                        (true, _, _) => MessageDialogButtons::OkCancel,
                    };
                    let result = tauri::async_runtime::spawn_blocking(move || {
                        app.dialog().message(text).title(title).kind(kind).buttons(buttons).blocking_show()
                    })
                    .await
                    .map_err(rt_err)?;
                    Ok(result)
                }
            })?,
        )?;
    }

    tauri.set("dialog", t)
}
