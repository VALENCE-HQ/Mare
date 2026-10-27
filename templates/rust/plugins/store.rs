// tauri.store: small persistent key-value files (JSON) in the app data folder.
//   local settings = tauri.store.open("settings.json")
//   settings:set("theme", "dark"); print(settings:get("theme"))
use std::sync::Arc;

use mlua::{Lua, LuaSerdeExt, Table, UserData, UserDataMethods, Value};
use tauri::{AppHandle, Wry};
use tauri_plugin_store::{Store, StoreExt};

use crate::mare::{rt_err, to_lua};

struct LuaStore(Arc<Store<Wry>>);

impl UserData for LuaStore {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_method("get", |lua, this, key: String| match this.0.get(key) {
            Some(v) => to_lua(lua, &v),
            None => Ok(Value::Nil),
        });
        m.add_method("set", |lua, this, (key, v): (String, Value)| {
            let json: serde_json::Value = lua.from_value(v)?;
            this.0.set(key, json);
            Ok(())
        });
        m.add_method("has", |_, this, key: String| Ok(this.0.has(key)));
        m.add_method("delete", |_, this, key: String| Ok(this.0.delete(key)));
        m.add_method("keys", |_, this, ()| Ok(this.0.keys()));
        m.add_method("clear", |_, this, ()| {
            this.0.clear();
            Ok(())
        });
        m.add_method("save", |_, this, ()| this.0.save().map_err(rt_err));
    }
}

pub fn install(app: &AppHandle, lua: &Lua, tauri: &Table) -> mlua::Result<()> {
    app.plugin(tauri_plugin_store::Builder::new().build()).map_err(rt_err)?;
    let t = lua.create_table()?;
    let a = app.clone();
    t.set(
        "open",
        lua.create_function(move |_, name: String| {
            if name.contains("..") || name.contains('/') || name.contains('\\') || name.contains(':') {
                return Err(rt_err("mare: store names must be plain file names like 'settings.json'"));
            }
            Ok(LuaStore(a.store(name).map_err(rt_err)?))
        })?,
    )?;
    tauri.set("store", t)
}
