// tauri.opener: open links and files in the user's default apps.
use mlua::{Lua, Table};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use crate::mare::{
    paths::{self, Access},
    rt_err,
};

pub fn install(app: &AppHandle, lua: &Lua, tauri: &Table) -> mlua::Result<()> {
    app.plugin(tauri_plugin_opener::init()).map_err(rt_err)?;
    let t = lua.create_table()?;

    {
        let a = app.clone();
        t.set(
            "open_url",
            lua.create_function(move |_, url: String| {
                let ok = ["http://", "https://", "mailto:", "tel:"].iter().any(|p| url.starts_with(p));
                if !ok {
                    return Err(rt_err("mare: only http, https, mailto and tel links can be opened"));
                }
                a.opener().open_url(url, None::<&str>).map_err(rt_err)
            })?,
        )?;
    }
    {
        let a = app.clone();
        t.set(
            "open_path",
            lua.create_function(move |_, path: String| {
                let p = paths::check(&a, &path, Access::Read).map_err(rt_err)?;
                a.opener().open_path(p.display().to_string(), None::<&str>).map_err(rt_err)
            })?,
        )?;
    }
    {
        let a = app.clone();
        t.set(
            "reveal",
            lua.create_function(move |_, path: String| {
                let p = paths::check(&a, &path, Access::Read).map_err(rt_err)?;
                a.opener().reveal_item_in_dir(p).map_err(rt_err)
            })?,
        )?;
    }
    tauri.set("opener", t)
}
