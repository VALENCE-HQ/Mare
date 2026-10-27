// tauri.autostart: launch the app when the user logs in.
use mlua::{Lua, Table};
use tauri::AppHandle;
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

use crate::mare::rt_err;

pub fn install(app: &AppHandle, lua: &Lua, tauri: &Table) -> mlua::Result<()> {
    app.plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None)).map_err(rt_err)?;
    let t = lua.create_table()?;
    {
        let a = app.clone();
        t.set("enable", lua.create_function(move |_, ()| a.autolaunch().enable().map_err(rt_err))?)?;
    }
    {
        let a = app.clone();
        t.set("disable", lua.create_function(move |_, ()| a.autolaunch().disable().map_err(rt_err))?)?;
    }
    {
        let a = app.clone();
        t.set("is_enabled", lua.create_function(move |_, ()| a.autolaunch().is_enabled().map_err(rt_err))?)?;
    }
    tauri.set("autostart", t)
}
