// `app` as seen from Lua: wrapper over tauri::AppHandle.
use mlua::{Function, LuaSerdeExt, Table, UserData, UserDataMethods, UserDataRef, Value};
use tauri::{AppHandle, Emitter, Listener, Manager};

use super::{
    json_str_to_lua,
    menu::LuaMenu,
    paths, rt_err,
    window::{self, LuaWindow},
    Callback,
};

#[derive(Clone)]
pub struct LuaApp(pub AppHandle);

pub struct LuaPaths(pub AppHandle);

const DIRS: &[(&str, &str)] = &[
    ("app_data_dir", "APPDATA"),
    ("app_config_dir", "APPCONFIG"),
    ("app_cache_dir", "APPCACHE"),
    ("app_log_dir", "APPLOG"),
    ("app_local_data_dir", "APPLOCALDATA"),
    ("resource_dir", "RESOURCE"),
    ("temp_dir", "TEMP"),
    ("home_dir", "HOME"),
    ("document_dir", "DOCUMENT"),
    ("download_dir", "DOWNLOAD"),
    ("desktop_dir", "DESKTOP"),
    ("picture_dir", "PICTURE"),
    ("audio_dir", "AUDIO"),
    ("video_dir", "VIDEO"),
];

impl UserData for LuaPaths {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        for &(method, var) in DIRS {
            m.add_method(method, move |_, this, ()| {
                paths::var_dir(&this.0, var)
                    .map(|p| p.display().to_string())
                    .map_err(rt_err)
            });
        }
        // path:resolve("$APPDATA/x.txt") -> real path string (does not grant access)
        m.add_method("resolve", |_, this, raw: String| {
            paths::expand(&this.0, &raw)
                .map(|p| p.display().to_string())
                .map_err(rt_err)
        });
    }
}

impl UserData for LuaApp {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_method("emit", |lua, this, (event, payload): (String, Value)| {
            let json: serde_json::Value = lua.from_value(payload)?;
            this.0.emit(&event, json).map_err(rt_err)
        });

        m.add_method(
            "emit_to",
            |lua, this, (label, event, payload): (String, String, Value)| {
                let json: serde_json::Value = lua.from_value(payload)?;
                this.0.emit_to(label.as_str(), &event, json).map_err(rt_err)
            },
        );

        m.add_method("listen", |lua, this, (event, cb): (String, Function)| {
            let cb = Callback::new(lua, cb);
            Ok(this.0.listen(event, move |e| {
                let lua = cb.lua();
                cb.fire(json_str_to_lua(&lua, e.payload()));
            }))
        });

        m.add_method("once", |lua, this, (event, cb): (String, Function)| {
            let cb = Callback::new(lua, cb);
            Ok(this.0.once(event, move |e| {
                let lua = cb.lua();
                cb.fire(json_str_to_lua(&lua, e.payload()));
            }))
        });

        m.add_method("unlisten", |_, this, id: u32| {
            this.0.unlisten(id);
            Ok(())
        });

        m.add_method("exit", |_, this, code: Option<i32>| {
            this.0.exit(code.unwrap_or(0));
            Ok(())
        });

        m.add_method("restart", |_, this, ()| -> mlua::Result<()> { this.0.restart() });

        m.add_method("eval_in", |_, this, (label, js): (String, String)| {
            if let Some(w) = this.0.get_webview_window(&label) {
                w.eval(&js).map_err(rt_err)?;
            }
            Ok(())
        });

        m.add_method("get_webview_window", |_, this, label: String| {
            Ok(this.0.get_webview_window(&label).map(LuaWindow))
        });

        m.add_method("windows", |_, this, ()| {
            Ok(this.0.webview_windows().keys().cloned().collect::<Vec<String>>())
        });

        m.add_method("create_window", |_, this, (label, opts): (String, Option<Table>)| {
            window::create_window(&this.0, &label, opts)
        });

        m.add_method("path", |_, this, ()| Ok(LuaPaths(this.0.clone())));

        m.add_method("package_info", |lua, this, ()| {
            let i = this.0.package_info();
            let t = lua.create_table()?;
            t.set("name", i.name.clone())?;
            t.set("version", i.version.to_string())?;
            t.set("authors", i.authors)?;
            t.set("description", i.description)?;
            t.set("crate_name", i.crate_name)?;
            Ok(t)
        });

        m.add_method("set_menu", |_, this, menu: UserDataRef<LuaMenu>| {
            this.0.set_menu(menu.0.clone()).map_err(rt_err)?;
            Ok(())
        });

        m.add_method("on_menu_event", |lua, this, cb: Function| {
            let cb = Callback::new(lua, cb);
            this.0.on_menu_event(move |_app, event| {
                let lua = cb.lua();
                if let Ok(t) = lua.create_table() {
                    let _ = t.set("id", event.id().0.clone());
                    cb.fire(t);
                }
            });
            Ok(())
        });
    }
}
