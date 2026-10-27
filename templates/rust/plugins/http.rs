// tauri.http.fetch(url, { method, headers, body, timeout }) -> { status, ok, headers, body }
// Hosts can be restricted with permissions.net in mare.json (default: any).
use std::time::Duration;

use mlua::{Lua, Table};
use tauri::AppHandle;
use tauri_plugin_http::reqwest;

use crate::mare::{config, opt, rt_err};

fn host_allowed(host: &str) -> bool {
    config::list_or(&["permissions", "net"], &["*"]).iter().any(|pat| {
        pat == "*"
            || pat == host
            || pat.strip_prefix("*.").map(|s| host.ends_with(&format!(".{s}"))).unwrap_or(false)
    })
}

pub fn install(_app: &AppHandle, lua: &Lua, tauri: &Table) -> mlua::Result<()> {
    let t = lua.create_table()?;
    t.set(
        "fetch",
        lua.create_async_function(|lua, (url, opts): (String, Option<Table>)| async move {
            let parsed = reqwest::Url::parse(&url).map_err(rt_err)?;
            if !matches!(parsed.scheme(), "http" | "https") {
                return Err(rt_err("mare: only http and https URLs are allowed"));
            }
            let host = parsed.host_str().unwrap_or_default().to_string();
            if !host_allowed(&host) {
                return Err(rt_err(format!("mare: requests to '{host}' are not allowed (permissions.net in mare.json)")));
            }

            let method = opts.as_ref().and_then(|o| opt::<String>(o, "method")).unwrap_or_else(|| "GET".into());
            let method = reqwest::Method::from_bytes(method.to_uppercase().as_bytes()).map_err(rt_err)?;
            let mut req = reqwest::Client::new().request(method, parsed);

            if let Some(o) = &opts {
                if let Some(h) = opt::<Table>(o, "headers") {
                    for (k, v) in h.pairs::<String, String>().flatten() {
                        req = req.header(k, v);
                    }
                }
                if let Some(b) = opt::<mlua::String>(o, "body") {
                    req = req.body(b.as_bytes().to_vec());
                }
                if let Some(secs) = opt::<f64>(o, "timeout") {
                    req = req.timeout(Duration::from_secs_f64(secs));
                }
            }

            let resp = req.send().await.map_err(rt_err)?;
            let status = resp.status();
            let headers = lua.create_table()?;
            for (k, v) in resp.headers() {
                headers.set(k.as_str().to_string(), String::from_utf8_lossy(v.as_bytes()).to_string())?;
            }
            let bytes = resp.bytes().await.map_err(rt_err)?;

            let out = lua.create_table()?;
            out.set("status", status.as_u16())?;
            out.set("ok", status.is_success())?;
            out.set("headers", headers)?;
            out.set("body", lua.create_string(&bytes)?)?;
            Ok(out)
        })?,
    )?;
    tauri.set("http", t)
}
