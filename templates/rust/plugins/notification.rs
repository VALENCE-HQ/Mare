// tauri.notification.send({ title, body })
use mlua::{Lua, Table};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

use crate::mare::{opt, rt_err};

pub fn install(app: &AppHandle, lua: &Lua, tauri: &Table) -> mlua::Result<()> {
    app.plugin(tauri_plugin_notification::init()).map_err(rt_err)?;
    let t = lua.create_table()?;
    let a = app.clone();
    t.set(
        "send",
        lua.create_function(move |_, o: Table| {
            let mut n = a.notification().builder();
            if let Some(v) = opt::<String>(&o, "title") { n = n.title(v); }
            if let Some(v) = opt::<String>(&o, "body") { n = n.body(v); }
            n.show().map_err(rt_err)
        })?,
    )?;
    tauri.set("notification", t)
}
