// Logging, JSON, channels, run events, and custom protocols.
use std::{
    borrow::Cow,
    fs::OpenOptions,
    io::Write,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use mlua::{Function, Lua, LuaSerdeExt, MultiValue, Table, UserData, UserDataMethods, Value};
use tauri::{http, AppHandle, Manager, RunEvent};

use super::{lookup, rt_err, to_lua, LuaRuntime};

// ---- channels: stream data to the frontend ---------------------------------------
//   tauri.command("download", function(ctx, args) args.on_event:send({ progress = 50 }) end)

pub struct LuaChannel(pub tauri::ipc::Channel<serde_json::Value>);

impl UserData for LuaChannel {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_method("send", |lua, this, v: Value| {
            let json: serde_json::Value = lua.from_value(v)?;
            this.0.send(json).map_err(rt_err)
        });
    }
}

// ---- logging ----------------------------------------------------------------------

fn write_log(app: &AppHandle, level: &str, msg: &str) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let line = format!("[{secs}] [{}] {msg}", level.to_uppercase());
    eprintln!("{line}");
    if let Ok(dir) = app.path().app_log_dir() {
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(dir.join("app.log")) {
            let _ = writeln!(f, "{line}");
        }
    }
}

pub fn install(app: &AppHandle, lua: &Lua, tauri: &Table) -> mlua::Result<()> {
    // tauri.log.info(...) etc. are defined in the Lua prelude on top of _write
    let log = lua.create_table()?;
    let a = app.clone();
    log.set(
        "_write",
        lua.create_function(move |_, (level, msg): (String, String)| {
            write_log(&a, &level, &msg);
            Ok(())
        })?,
    )?;
    tauri.set("log", log)?;

    let json = lua.create_table()?;
    json.set(
        "encode",
        lua.create_function(|lua, (v, pretty): (Value, Option<bool>)| {
            let j: serde_json::Value = lua.from_value(v)?;
            let out = if pretty.unwrap_or(false) {
                serde_json::to_string_pretty(&j)
            } else {
                serde_json::to_string(&j)
            };
            out.map_err(rt_err)
        })?,
    )?;
    json.set(
        "decode",
        lua.create_function(|lua, s: String| {
            let v: serde_json::Value = serde_json::from_str(&s).map_err(rt_err)?;
            to_lua(lua, &v)
        })?,
    )?;
    tauri.set("json", json)
}

// ---- run events -------------------------------------------------------------------

pub fn dispatch_run_event(app: &AppHandle, event: &RunEvent) {
    let Some(rt) = app.try_state::<Arc<LuaRuntime>>() else { return };
    let rt: Arc<LuaRuntime> = rt.inner().clone();
    let cb: Option<Function> = {
        let g = rt.reg.run_event.lock().unwrap();
        g.as_ref().and_then(|k| rt.lua.registry_value::<Function>(k).ok())
    };
    let Some(cb) = cb else { return };
    let lua = &rt.lua;
    let Ok(t) = lua.create_table() else { return };
    let prevent = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let built = (|| -> mlua::Result<bool> {
        match event {
            RunEvent::Ready => t.set("type", "ready")?,
            RunEvent::Exit => t.set("type", "exit")?,
            RunEvent::Resumed => t.set("type", "resumed")?,
            RunEvent::ExitRequested { code, .. } => {
                t.set("type", "exit_requested")?;
                t.set("code", *code)?;
                let flag = prevent.clone();
                t.set(
                    "prevent_exit",
                    lua.create_function(move |_, _: MultiValue| {
                        flag.store(true, std::sync::atomic::Ordering::SeqCst);
                        Ok(())
                    })?,
                )?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    })()
    .unwrap_or(false);
    if !built {
        return;
    }

    let result = cb.call::<Value>(t);
    let returned_false = matches!(result, Ok(Value::Boolean(false)));
    if let Err(e) = &result {
        eprintln!("[mare] on_run_event failed: {e}");
    }
    if let RunEvent::ExitRequested { api, .. } = event {
        if returned_false || prevent.load(std::sync::atomic::Ordering::SeqCst) {
            api.prevent_exit();
        }
    }
}

// ---- custom protocols -------------------------------------------------------------
//   tauri.register_protocol("app", function(ctx, request) return { status = 200, body = "hi" } end)
//   (list the scheme under "protocols" in mare.json too)

fn plain(status: u16, msg: String) -> http::Response<Cow<'static, [u8]>> {
    http::Response::builder()
        .status(status)
        .header("Content-Type", "text/plain")
        .body(Cow::Owned(msg.into_bytes()))
        .unwrap()
}

pub async fn handle_protocol(
    app: AppHandle,
    scheme: String,
    request: http::Request<Vec<u8>>,
) -> http::Response<Cow<'static, [u8]>> {
    let rt: Arc<LuaRuntime> = match app.try_state::<Arc<LuaRuntime>>() {
        Some(s) => s.inner().clone(),
        None => return plain(503, "app is starting".into()),
    };
    let Some(func) = lookup(&rt.lua, &rt.reg.protocols, &scheme) else {
        return plain(404, format!("no Lua handler for '{scheme}'"));
    };

    let build = || -> mlua::Result<(Table, Table)> {
        let lua = &rt.lua;
        let ctx = lua.create_table()?;
        ctx.set("scheme", scheme.clone())?;
        let req = lua.create_table()?;
        req.set("uri", request.uri().to_string())?;
        req.set("method", request.method().as_str())?;
        let headers = lua.create_table()?;
        for (k, v) in request.headers() {
            headers.set(k.as_str().to_lowercase(), String::from_utf8_lossy(v.as_bytes()).to_string())?;
        }
        req.set("headers", headers)?;
        req.set("body", lua.create_string(request.body())?)?;
        Ok((ctx, req))
    };
    let (ctx, req) = match build() {
        Ok(v) => v,
        Err(e) => return plain(500, e.to_string()),
    };

    let ret: Value = match func.call_async((ctx, req)).await {
        Ok(v) => v,
        Err(e) => return plain(500, e.to_string()),
    };
    let Value::Table(resp) = ret else {
        return plain(500, "Lua protocol handler must return a table".into());
    };

    let status = super::opt::<u16>(&resp, "status").unwrap_or(200);
    let mut b = http::Response::builder().status(status);
    if let Some(h) = super::opt::<Table>(&resp, "headers") {
        for pair in h.pairs::<String, String>().flatten() {
            b = b.header(pair.0, pair.1);
        }
    }
    let body: Vec<u8> = match super::opt::<mlua::String>(&resp, "body") {
        Some(s) => s.as_bytes().to_vec(),
        None => Vec::new(),
    };
    b.body(Cow::Owned(body))
        .unwrap_or_else(|e| plain(500, e.to_string()))
}
