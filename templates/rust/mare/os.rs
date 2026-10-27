// tauri.os: process and environment access, permission-checked (permissions.os in mare.json).
use std::sync::atomic::{AtomicU64, Ordering};

use mlua::{Lua, Table};
use tauri::AppHandle;

use super::{
    config,
    paths::{self, Access},
    opt, rt_err,
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn install(app: &AppHandle, lua: &Lua, tauri: &Table) -> mlua::Result<()> {
    let t = lua.create_table()?;

    t.set("platform", lua.create_function(|_, ()| Ok(std::env::consts::OS))?)?;
    t.set("arch", lua.create_function(|_, ()| Ok(std::env::consts::ARCH))?)?;

    // Replaces os.getenv. Only variables listed in permissions.os.env are readable.
    t.set(
        "getenv",
        lua.create_function(|_, name: String| {
            let allowed = config::list_or(&["permissions", "os", "env"], &["HOME", "USER", "USERNAME", "LANG"]);
            if !allowed.iter().any(|a| a == &name) {
                return Err(rt_err(format!(
                    "mare: reading ${name} is not allowed (permissions.os.env in mare.json)"
                )));
            }
            Ok(std::env::var(&name).ok())
        })?,
    )?;

    // Replaces os.tmpname: a unique path inside the app's temp folder.
    {
        let a = app.clone();
        t.set(
            "tmpname",
            lua.create_function(move |_, ()| {
                let n = COUNTER.fetch_add(1, Ordering::SeqCst);
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                let p = paths::var_dir(&a, "TEMP").map_err(rt_err)?.join(format!("mare-{nanos}-{n}"));
                Ok(p.display().to_string())
            })?,
        )?;
    }

    // Replaces os.exit: quits through Tauri so cleanup runs.
    {
        let a = app.clone();
        t.set(
            "exit",
            lua.create_function(move |_, code: Option<i32>| {
                a.exit(code.unwrap_or(0));
                Ok(())
            })?,
        )?;
    }

    // Replaces os.execute / io.popen. Programs must be listed in permissions.os.exec.
    {
        let a = app.clone();
        t.set(
            "execute",
            lua.create_async_function(move |lua, (program, args, opts): (String, Option<Vec<String>>, Option<Table>)| {
                let a = a.clone();
                async move {
                    let allowed = config::list_or(&["permissions", "os", "exec"], &[]);
                    if !allowed.iter().any(|p| p == &program) {
                        return Err(rt_err(format!(
                            "mare: running '{program}' is not allowed (permissions.os.exec in mare.json)"
                        )));
                    }
                    let cwd = match opts.and_then(|o| opt::<String>(&o, "cwd")) {
                        Some(c) => Some(paths::check(&a, &c, Access::Read).map_err(rt_err)?),
                        None => None,
                    };
                    let out = tauri::async_runtime::spawn_blocking(move || {
                        let mut cmd = std::process::Command::new(&program);
                        cmd.args(args.unwrap_or_default());
                        if let Some(c) = cwd {
                            cmd.current_dir(c);
                        }
                        cmd.output()
                    })
                    .await
                    .map_err(rt_err)?
                    .map_err(rt_err)?;

                    let row = lua.create_table()?;
                    row.set("code", out.status.code())?;
                    row.set("success", out.status.success())?;
                    row.set("stdout", lua.create_string(&out.stdout)?)?;
                    row.set("stderr", lua.create_string(&out.stderr)?)?;
                    Ok(row)
                }
            })?,
        )?;
    }

    tauri.set("os", t)
}
