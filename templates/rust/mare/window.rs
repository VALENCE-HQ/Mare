// Windows: `ctx.window`, `app:get_webview_window(label)`, `tauri.window.new(label, opts)`.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use mlua::{
    Function, Lua, LuaSerdeExt, MultiValue, Table, UserData, UserDataMethods, Value,
};
use tauri::{
    window::Monitor, AppHandle, DragDropEvent, Emitter, Listener, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

use super::{json_str_to_lua, opt, paths, rt_err, Callback};

#[derive(Clone)]
pub struct LuaWindow(pub WebviewWindow);

macro_rules! unit_methods {
    ($m:ident, $($name:literal => $f:ident),* $(,)?) => {
        $( $m.add_method($name, |_, this, ()| this.0.$f().map_err(rt_err)); )*
    };
}
macro_rules! flag_methods {
    ($m:ident, $($name:literal => $f:ident),* $(,)?) => {
        $( $m.add_method($name, |_, this, v: bool| this.0.$f(v).map_err(rt_err)); )*
    };
}
macro_rules! query_methods {
    ($m:ident, $($name:literal => $f:ident),* $(,)?) => {
        $( $m.add_method($name, |_, this, ()| this.0.$f().map_err(rt_err)); )*
    };
}

fn monitor_table(lua: &Lua, m: &Monitor) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("name", m.name().cloned())?;
    t.set("width", m.size().width)?;
    t.set("height", m.size().height)?;
    t.set("x", m.position().x)?;
    t.set("y", m.position().y)?;
    t.set("scale_factor", m.scale_factor())?;
    Ok(t)
}

fn dispatch_window_event(cb: &Callback, event: &WindowEvent) {
    let lua = cb.lua();
    let Ok(t) = lua.create_table() else { return };
    let prevent = Arc::new(AtomicBool::new(false));

    let ok = (|| -> mlua::Result<bool> {
        match event {
            WindowEvent::CloseRequested { .. } => {
                t.set("type", "close_requested")?;
                let flag = prevent.clone();
                t.set(
                    "prevent_close",
                    lua.create_function(move |_, _: MultiValue| {
                        flag.store(true, Ordering::SeqCst);
                        Ok(())
                    })?,
                )?;
            }
            WindowEvent::Resized(s) => {
                t.set("type", "resized")?;
                t.set("width", s.width)?;
                t.set("height", s.height)?;
            }
            WindowEvent::Moved(p) => {
                t.set("type", "moved")?;
                t.set("x", p.x)?;
                t.set("y", p.y)?;
            }
            WindowEvent::Focused(f) => {
                t.set("type", "focused")?;
                t.set("focused", *f)?;
            }
            WindowEvent::Destroyed => t.set("type", "destroyed")?,
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                t.set("type", "scale_factor_changed")?;
                t.set("scale_factor", *scale_factor)?;
            }
            WindowEvent::ThemeChanged(theme) => {
                t.set("type", "theme_changed")?;
                t.set("theme", format!("{theme:?}").to_lowercase())?;
            }
            WindowEvent::DragDrop(d) => match d {
                DragDropEvent::Drop { paths: dropped, .. } => {
                    // Files the user dropped on the window become readable
                    for p in dropped {
                        paths::grant(p);
                    }
                    t.set("type", "drop")?;
                    t.set(
                        "paths",
                        dropped.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
                    )?;
                }
                DragDropEvent::Enter { .. } => t.set("type", "drag_enter")?,
                DragDropEvent::Leave => t.set("type", "drag_leave")?,
                _ => return Ok(false),
            },
            _ => return Ok(false),
        }
        Ok(true)
    })()
    .unwrap_or(false);
    if !ok {
        return;
    }

    let result = cb.fire(t);
    let returned_false = matches!(result, Some(Value::Boolean(false)));
    if let WindowEvent::CloseRequested { api, .. } = event {
        if returned_false || prevent.load(Ordering::SeqCst) {
            api.prevent_close();
        }
    }
}

impl UserData for LuaWindow {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        unit_methods!(m,
            "show" => show, "hide" => hide, "close" => close, "destroy" => destroy,
            "minimize" => minimize, "unminimize" => unminimize,
            "maximize" => maximize, "unmaximize" => unmaximize,
            "set_focus" => set_focus, "center" => center,
        );
        flag_methods!(m,
            "set_always_on_top" => set_always_on_top, "set_fullscreen" => set_fullscreen,
            "set_resizable" => set_resizable, "set_decorations" => set_decorations,
            "set_skip_taskbar" => set_skip_taskbar,
            "set_ignore_cursor_events" => set_ignore_cursor_events,
        );
        query_methods!(m,
            "is_visible" => is_visible, "is_maximized" => is_maximized,
            "is_minimized" => is_minimized, "is_fullscreen" => is_fullscreen,
            "is_focused" => is_focused, "is_decorated" => is_decorated,
            "is_resizable" => is_resizable, "title" => title,
        );

        m.add_method("label", |_, this, ()| Ok(this.0.label().to_string()));
        m.add_method("set_title", |_, this, t: String| this.0.set_title(&t).map_err(rt_err));

        m.add_method("set_size", |_, this, (w, h): (f64, f64)| {
            this.0.set_size(tauri::LogicalSize::new(w, h)).map_err(rt_err)
        });
        m.add_method("set_position", |_, this, (x, y): (f64, f64)| {
            this.0.set_position(tauri::LogicalPosition::new(x, y)).map_err(rt_err)
        });
        m.add_method("set_min_size", |_, this, (w, h): (f64, f64)| {
            this.0.set_min_size(Some(tauri::LogicalSize::new(w, h))).map_err(rt_err)
        });
        m.add_method("set_max_size", |_, this, (w, h): (f64, f64)| {
            this.0.set_max_size(Some(tauri::LogicalSize::new(w, h))).map_err(rt_err)
        });

        m.add_method("inner_size", |lua, this, ()| {
            let s = this.0.inner_size().map_err(rt_err)?;
            let t = lua.create_table()?;
            t.set("width", s.width)?;
            t.set("height", s.height)?;
            Ok(t)
        });
        m.add_method("outer_position", |lua, this, ()| {
            let p = this.0.outer_position().map_err(rt_err)?;
            let t = lua.create_table()?;
            t.set("x", p.x)?;
            t.set("y", p.y)?;
            Ok(t)
        });
        m.add_method("scale_factor", |_, this, ()| this.0.scale_factor().map_err(rt_err));

        m.add_method("monitors", |lua, this, ()| {
            let list = this.0.available_monitors().map_err(rt_err)?;
            list.iter().map(|m| monitor_table(lua, m)).collect::<mlua::Result<Vec<_>>>()
        });
        m.add_method("primary_monitor", |lua, this, ()| {
            match this.0.primary_monitor().map_err(rt_err)? {
                Some(mon) => Ok(Some(monitor_table(lua, &mon)?)),
                None => Ok(None),
            }
        });

        m.add_method("eval", |_, this, js: String| this.0.eval(&js).map_err(rt_err));

        m.add_method("open_devtools", |_, this, ()| {
            #[cfg(debug_assertions)]
            this.0.open_devtools();
            #[cfg(not(debug_assertions))]
            let _ = this;
            Ok(())
        });
        m.add_method("close_devtools", |_, this, ()| {
            #[cfg(debug_assertions)]
            this.0.close_devtools();
            #[cfg(not(debug_assertions))]
            let _ = this;
            Ok(())
        });

        // Events sent to this window only
        m.add_method("emit", |lua, this, (event, payload): (String, Value)| {
            let json: serde_json::Value = lua.from_value(payload)?;
            this.0.emit_to(this.0.label(), &event, json).map_err(rt_err)
        });
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

        // Return false from the callback (or call event:prevent_close()) to keep a window open
        m.add_method("on_window_event", |lua, this, cb: Function| {
            let cb = Callback::new(lua, cb);
            this.0.on_window_event(move |event| dispatch_window_event(&cb, event));
            Ok(())
        });
    }
}

pub fn create_window(app: &AppHandle, label: &str, opts: Option<Table>) -> mlua::Result<LuaWindow> {
    fn g<T: mlua::FromLua>(o: &Option<Table>, key: &str) -> Option<T> {
        o.as_ref().and_then(|t| opt::<T>(t, key))
    }
    let o = opts;

    let url = g::<String>(&o, "url").unwrap_or_else(|| "index.html".to_string());
    let target = if url.starts_with("http://") || url.starts_with("https://") {
        WebviewUrl::External(url.parse().map_err(rt_err)?)
    } else {
        WebviewUrl::App(url.into())
    };

    let mut b = WebviewWindowBuilder::new(app, label, target);
    if let Some(v) = g::<String>(&o, "title") { b = b.title(v); }
    if let (Some(w), Some(h)) = (g::<f64>(&o, "width"), g::<f64>(&o, "height")) { b = b.inner_size(w, h); }
    if let (Some(x), Some(y)) = (g::<f64>(&o, "x"), g::<f64>(&o, "y")) { b = b.position(x, y); }
    if let (Some(w), Some(h)) = (g::<f64>(&o, "min_width"), g::<f64>(&o, "min_height")) { b = b.min_inner_size(w, h); }
    if let (Some(w), Some(h)) = (g::<f64>(&o, "max_width"), g::<f64>(&o, "max_height")) { b = b.max_inner_size(w, h); }
    if let Some(v) = g::<bool>(&o, "resizable") { b = b.resizable(v); }
    if let Some(v) = g::<bool>(&o, "decorations") { b = b.decorations(v); }
    if let Some(v) = g::<bool>(&o, "transparent") { b = b.transparent(v); }
    if let Some(v) = g::<bool>(&o, "always_on_top") { b = b.always_on_top(v); }
    if let Some(v) = g::<bool>(&o, "fullscreen") { b = b.fullscreen(v); }
    if let Some(v) = g::<bool>(&o, "visible") { b = b.visible(v); }
    if let Some(v) = g::<bool>(&o, "maximized") { b = b.maximized(v); }
    if let Some(v) = g::<bool>(&o, "focused") { b = b.focused(v); }
    if let Some(v) = g::<bool>(&o, "skip_taskbar") { b = b.skip_taskbar(v); }
    if g::<bool>(&o, "center").unwrap_or(false) { b = b.center(); }

    Ok(LuaWindow(b.build().map_err(rt_err)?))
}

pub fn install(app: &AppHandle, lua: &Lua, tauri: &Table) -> mlua::Result<()> {
    let w = lua.create_table()?;
    let a = app.clone();
    // tauri.window.new("settings", { url = "settings.html", width = 600, height = 400 })
    w.set(
        "new",
        lua.create_function(move |_, (label, opts): (String, Option<Table>)| {
            create_window(&a, &label, opts)
        })?,
    )?;
    tauri.set("window", w)
}
