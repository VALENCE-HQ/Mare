// tauri.clipboard: read and write text.
use mlua::{Lua, Table};
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

use crate::mare::rt_err;

pub fn install(app: &AppHandle, lua: &Lua, tauri: &Table) -> mlua::Result<()> {
    app.plugin(tauri_plugin_clipboard_manager::init()).map_err(rt_err)?;
    let t = lua.create_table()?;
    {
        let a = app.clone();
        t.set("read_text", lua.create_function(move |_, ()| a.clipboard().read_text().map_err(rt_err))?)?;
    }
    {
        let a = app.clone();
        t.set("write_text", lua.create_function(move |_, s: String| a.clipboard().write_text(s).map_err(rt_err))?)?;
    }
    {
        let a = app.clone();
        t.set("clear", lua.create_function(move |_, ()| a.clipboard().clear().map_err(rt_err))?)?;
    }
    tauri.set("clipboard", t)
}
