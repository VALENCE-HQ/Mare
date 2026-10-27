// tauri.global_shortcut: hotkeys that work even when the app isn't focused.
//   tauri.global_shortcut.register("CmdOrCtrl+Shift+K", function(e) print(e.state) end)
use mlua::{Function, Lua, Table};
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::mare::{rt_err, Callback};

pub fn install(app: &AppHandle, lua: &Lua, tauri: &Table) -> mlua::Result<()> {
    app.plugin(tauri_plugin_global_shortcut::Builder::new().build()).map_err(rt_err)?;
    let t = lua.create_table()?;

    {
        let a = app.clone();
        t.set(
            "register",
            lua.create_function(move |lua, (accel, cb): (String, Function)| {
                let cb = Callback::new(lua, cb);
                let name = accel.clone();
                a.global_shortcut()
                    .on_shortcut(accel.as_str(), move |_app, _shortcut, event| {
                        let lua = cb.lua();
                        if let Ok(t) = lua.create_table() {
                            let _ = t.set("shortcut", name.clone());
                            let state = match event.state() {
                                ShortcutState::Pressed => "pressed",
                                ShortcutState::Released => "released",
                            };
                            let _ = t.set("state", state);
                            cb.fire(t);
                        }
                    })
                    .map_err(rt_err)
            })?,
        )?;
    }
    {
        let a = app.clone();
        t.set("unregister", lua.create_function(move |_, accel: String| {
            a.global_shortcut().unregister(accel.as_str()).map_err(rt_err)
        })?)?;
    }
    {
        let a = app.clone();
        t.set("unregister_all", lua.create_function(move |_, ()| {
            a.global_shortcut().unregister_all().map_err(rt_err)
        })?)?;
    }
    {
        let a = app.clone();
        t.set("is_registered", lua.create_function(move |_, accel: String| {
            Ok(a.global_shortcut().is_registered(accel.as_str()))
        })?)?;
    }
    tauri.set("global_shortcut", t)
}
