// tauri.updater: check for and install app updates.
// Needs a signing key and endpoints in tauri.conf.json (plugins.updater). See Tauri's updater docs.
//   local update = tauri.updater.check()
//   if update then tauri.updater.install(function(done, total) ... end); app:restart() end
use std::sync::Mutex;

use mlua::{Function, Lua, Table, Value};
use tauri::AppHandle;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::mare::{rt_err, Callback};

static PENDING: Mutex<Option<Update>> = Mutex::new(None);

pub fn install(app: &AppHandle, lua: &Lua, tauri: &Table) -> mlua::Result<()> {
    app.plugin(tauri_plugin_updater::Builder::new().build()).map_err(rt_err)?;
    let t = lua.create_table()?;

    {
        let a = app.clone();
        t.set(
            "check",
            lua.create_async_function(move |lua, ()| {
                let a = a.clone();
                async move {
                    let updater = a.updater().map_err(rt_err)?;
                    match updater.check().await.map_err(rt_err)? {
                        Some(update) => {
                            let row = lua.create_table()?;
                            row.set("version", update.version.clone())?;
                            row.set("current_version", update.current_version.clone())?;
                            row.set("notes", update.body.clone())?;
                            *PENDING.lock().unwrap() = Some(update);
                            Ok(Value::Table(row))
                        }
                        None => Ok(Value::Nil),
                    }
                }
            })?,
        )?;
    }

    t.set(
        "install",
        lua.create_async_function(|lua, progress: Option<Function>| async move {
            let update = PENDING.lock().unwrap().take();
            let Some(update) = update else {
                return Err(rt_err("mare: call tauri.updater.check() first"));
            };
            let cb = progress.map(|f| Callback::new(&lua, f));
            let mut downloaded: u64 = 0;
            update
                .download_and_install(
                    move |chunk, total| {
                        downloaded += chunk as u64;
                        if let Some(cb) = &cb {
                            cb.fire((downloaded, total));
                        }
                    },
                    || {},
                )
                .await
                .map_err(rt_err)
        })?,
    )?;
    tauri.set("updater", t)
}
