// MANAGED BY MARE. `mare sync` overwrites this folder. Don't edit.
#![allow(dead_code)]

pub mod app;
pub mod config;
pub mod io;
pub mod menu;
pub mod misc;
pub mod os;
pub mod paths;
pub mod plugins;
pub mod sandbox;
pub mod window;

use std::{
    collections::HashMap,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
};

use mlua::{
    FromLua, Function, IntoLuaMulti, Lua, LuaSerdeExt, RegistryKey, SerializeOptions, Table, Value,
};
use tauri::{
    ipc::{InvokeBody, InvokeError, JavaScriptChannelId},
    AppHandle, Manager,
};

use app::LuaApp;
use window::LuaWindow;

// ---- small helpers used everywhere -------------------------------------------

pub fn rt_err<E: ToString>(e: E) -> mlua::Error {
    mlua::Error::runtime(e.to_string())
}

/// Read an optional field from a Lua table (nil or wrong type -> None).
pub fn opt<T: FromLua>(t: &Table, key: &str) -> Option<T> {
    t.get::<Option<T>>(key).ok().flatten()
}

/// JSON -> Lua. JSON null becomes nil (not a sentinel value).
pub fn to_lua(lua: &Lua, v: &serde_json::Value) -> mlua::Result<Value> {
    lua.to_value_with(
        v,
        SerializeOptions::new()
            .serialize_none_to_null(false)
            .serialize_unit_to_null(false),
    )
}

/// Event payloads arrive as JSON text; hand Lua a real value.
pub fn json_str_to_lua(lua: &Lua, s: &str) -> Value {
    match serde_json::from_str::<serde_json::Value>(s) {
        Ok(v) => to_lua(lua, &v).unwrap_or(Value::Nil),
        Err(_) => lua.create_string(s).map(Value::String).unwrap_or(Value::Nil),
    }
}

/// A Lua function that can be handed to Tauri callbacks (which need Send + Sync).
/// The lock is only held while cloning, so re-entrant calls can't deadlock.
#[derive(Clone)]
pub struct Callback(Arc<Mutex<(Lua, Function)>>);

impl Callback {
    pub fn new(lua: &Lua, f: Function) -> Self {
        Callback(Arc::new(Mutex::new((lua.clone(), f))))
    }
    pub fn lua(&self) -> Lua {
        self.0.lock().unwrap().0.clone()
    }
    pub fn call<A: IntoLuaMulti>(&self, args: A) -> mlua::Result<Value> {
        let f = self.0.lock().unwrap().1.clone();
        f.call::<Value>(args)
    }
    /// Call and print (not propagate) any Lua error.
    pub fn fire<A: IntoLuaMulti>(&self, args: A) -> Option<Value> {
        match self.call(args) {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("[mare] Lua callback failed: {e}");
                None
            }
        }
    }
}

// ---- runtime -------------------------------------------------------------------

#[derive(Default)]
pub struct Registry {
    pub commands: Mutex<HashMap<String, RegistryKey>>,
    pub protocols: Mutex<HashMap<String, RegistryKey>>,
    pub run_event: Mutex<Option<RegistryKey>>,
    pub setup: Mutex<Option<RegistryKey>>,
}

pub struct LuaRuntime {
    pub lua: Lua,
    pub reg: Arc<Registry>,
}

pub fn lookup(lua: &Lua, map: &Mutex<HashMap<String, RegistryKey>>, name: &str) -> Option<Function> {
    let g = map.lock().unwrap();
    let key = g.get(name)?;
    lua.registry_value::<Function>(key).ok()
}

// ---- the global `tauri` table ----------------------------------------------------

fn build_api(app: &AppHandle, lua: &Lua, reg: Arc<Registry>) -> mlua::Result<()> {
    let tauri: Table = lua.create_table()?;

    {
        let reg = reg.clone();
        tauri.set(
            "command",
            lua.create_function(move |lua, (name, f): (String, Function)| {
                let key = lua.create_registry_value(f)?;
                reg.commands.lock().unwrap().insert(name, key);
                Ok(())
            })?,
        )?;
    }
    {
        let reg = reg.clone();
        tauri.set(
            "setup",
            lua.create_function(move |lua, f: Function| {
                *reg.setup.lock().unwrap() = Some(lua.create_registry_value(f)?);
                Ok(())
            })?,
        )?;
    }
    {
        let reg = reg.clone();
        tauri.set(
            "on_run_event",
            lua.create_function(move |lua, f: Function| {
                *reg.run_event.lock().unwrap() = Some(lua.create_registry_value(f)?);
                Ok(())
            })?,
        )?;
    }
    {
        let reg = reg.clone();
        tauri.set(
            "register_protocol",
            lua.create_function(move |lua, (scheme, f): (String, Function)| {
                let key = lua.create_registry_value(f)?;
                reg.protocols.lock().unwrap().insert(scheme, key);
                Ok(())
            })?,
        )?;
    }

    tauri.set(
        "sleep",
        lua.create_async_function(|_, secs: f64| async move {
            tokio::time::sleep(std::time::Duration::from_secs_f64(secs)).await;
            Ok(())
        })?,
    )?;

    tauri.set(
        "spawn",
        lua.create_function(|_, f: Function| {
            tauri::async_runtime::spawn(async move {
                if let Err(e) = f.call_async::<()>(()).await {
                    eprintln!("[mare] spawned task failed: {e}");
                }
            });
            Ok(())
        })?,
    )?;

    misc::install(app, lua, &tauri)?;
    window::install(app, lua, &tauri)?;
    menu::install(app, lua, &tauri)?;
    io::install(app, lua, &tauri)?;
    os::install(app, lua, &tauri)?;
    plugins::install(app, lua, &tauri)?;

    lua.globals().set("tauri", tauri)?;
    Ok(())
}

// ---- routing frontend commands into Lua -----------------------------------------

fn err(e: impl ToString) -> String {
    e.to_string()
}

async fn call_lua(
    app: AppHandle,
    webview: tauri::Webview,
    rt: Arc<LuaRuntime>,
    name: String,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let func = lookup(&rt.lua, &rt.reg.commands, &name)
        .ok_or_else(|| format!("no Lua command '{name}'"))?;
    let lua = &rt.lua;

    // ctx carries what #[tauri::command] would normally inject
    let ctx = lua.create_table().map_err(err)?;
    ctx.set("app", LuaApp(app.clone())).map_err(err)?;
    ctx.set("webview_label", webview.label().to_string()).map_err(err)?;
    if let Some(w) = app.get_webview_window(webview.label()) {
        ctx.set("window", LuaWindow(w)).map_err(err)?;
    }
    let tauri_t: Table = lua.globals().get("tauri").map_err(err)?;
    ctx.set("state", tauri_t.get::<Function>("state").map_err(err)?)
        .map_err(err)?;

    let lua_args = to_lua(lua, &args).map_err(err)?;

    // JS `Channel` objects arrive as "__CHANNEL__:<id>" strings: turn them into channels
    if let Value::Table(t) = &lua_args {
        let mut chans = Vec::new();
        for pair in t.clone().pairs::<String, Value>() {
            let (k, v) = pair.map_err(err)?;
            if let Value::String(s) = &v {
                if let Ok(st) = s.to_str() {
                    if let Ok(id) = JavaScriptChannelId::from_str(&st) {
                        chans.push((k, id));
                    }
                }
            }
        }
        for (k, id) in chans {
            let ch: tauri::ipc::Channel<serde_json::Value> = id.channel_on(webview.clone());
            t.set(k, misc::LuaChannel(ch)).map_err(err)?;
        }
    }

    let ret: Value = func.call_async((ctx, lua_args)).await.map_err(err)?;
    lua.from_value(ret).map_err(err)
}

/// Any command name the frontend invokes is routed to Lua if Lua registered it.
pub fn invoke_handler(invoke: tauri::ipc::Invoke) -> bool {
    let name = invoke.message.command().to_string();
    let webview = invoke.message.webview();
    let app = webview.app_handle().clone();

    let rt = match app.try_state::<Arc<LuaRuntime>>() {
        Some(s) => s.inner().clone(),
        None => return false,
    };
    if !rt.reg.commands.lock().unwrap().contains_key(&name) {
        return false;
    }

    let args = match invoke.message.payload() {
        InvokeBody::Json(v) => v.clone(),
        _ => serde_json::Value::Null,
    };

    invoke.resolver.respond_async(async move {
        call_lua(app, webview, rt, name, args)
            .await
            .map_err(InvokeError::from)
    });
    true
}

// ---- startup ---------------------------------------------------------------------

pub fn lua_dir(app: &AppHandle) -> PathBuf {
    if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../lua")
    } else {
        app.path().resource_dir().expect("no resource dir").join("lua")
    }
}

pub fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let dir = lua_dir(&handle);
    let reg = Arc::new(Registry::default());
    let lua = sandbox::new_lua()?;

    // `require` may only load modules from the project's lua/ folder
    let path = format!("{0}/?.lua;{0}/?/init.lua", dir.display());
    lua.load(format!("package.path = [[{path}]]")).exec()?;

    build_api(&handle, &lua, reg.clone())?;
    sandbox::prelude(&lua)?;

    app.manage(Arc::new(LuaRuntime {
        lua: lua.clone(),
        reg: reg.clone(),
    }));

    lua.load(dir.join("main.lua")).exec().map_err(|e| {
        eprintln!("[mare] error in lua/main.lua:\n{e}");
        e
    })?;

    let setup_cb = {
        let g = reg.setup.lock().unwrap();
        match g.as_ref() {
            Some(k) => Some(lua.registry_value::<Function>(k)?),
            None => None,
        }
    };
    if let Some(f) = setup_cb {
        f.call::<()>(LuaApp(handle.clone()))?;
    }
    Ok(())
}
