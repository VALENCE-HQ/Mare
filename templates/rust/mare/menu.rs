// Menus and the tray icon, built from plain Lua tables.
//
//   local menu = tauri.menu.new({
//     { label = "File", items = {
//         { id = "open", label = "Open...", accelerator = "CmdOrCtrl+O" },
//         "separator",
//         "quit",
//     }},
//   })
//   app:set_menu(menu)
use mlua::{Function, Lua, Table, UserData, UserDataMethods, UserDataRef, Value};
use tauri::{
    menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::{TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Wry,
};

use super::{opt, rt_err, Callback};

pub struct LuaMenu(pub Menu<Wry>);
impl UserData for LuaMenu {}

pub struct LuaTray(pub TrayIcon<Wry>);
impl UserData for LuaTray {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_method("set_tooltip", |_, this, tip: Option<String>| {
            this.0.set_tooltip(tip).map_err(rt_err)
        });
        m.add_method("set_visible", |_, this, v: bool| this.0.set_visible(v).map_err(rt_err));
    }
}

type Item = Box<dyn IsMenuItem<Wry>>;

fn predefined(app: &AppHandle, name: &str) -> mlua::Result<Item> {
    let item: Item = match name {
        "separator" => Box::new(PredefinedMenuItem::separator(app).map_err(rt_err)?),
        "copy" => Box::new(PredefinedMenuItem::copy(app, None).map_err(rt_err)?),
        "cut" => Box::new(PredefinedMenuItem::cut(app, None).map_err(rt_err)?),
        "paste" => Box::new(PredefinedMenuItem::paste(app, None).map_err(rt_err)?),
        "select_all" => Box::new(PredefinedMenuItem::select_all(app, None).map_err(rt_err)?),
        "undo" => Box::new(PredefinedMenuItem::undo(app, None).map_err(rt_err)?),
        "redo" => Box::new(PredefinedMenuItem::redo(app, None).map_err(rt_err)?),
        "minimize" => Box::new(PredefinedMenuItem::minimize(app, None).map_err(rt_err)?),
        "maximize" => Box::new(PredefinedMenuItem::maximize(app, None).map_err(rt_err)?),
        "fullscreen" => Box::new(PredefinedMenuItem::fullscreen(app, None).map_err(rt_err)?),
        "hide" => Box::new(PredefinedMenuItem::hide(app, None).map_err(rt_err)?),
        "close_window" => Box::new(PredefinedMenuItem::close_window(app, None).map_err(rt_err)?),
        "quit" => Box::new(PredefinedMenuItem::quit(app, None).map_err(rt_err)?),
        other => return Err(rt_err(format!("unknown predefined menu item '{other}'"))),
    };
    Ok(item)
}

fn from_table(app: &AppHandle, t: &Table) -> mlua::Result<Item> {
    let label: String = opt(t, "label").unwrap_or_default();
    let enabled = opt::<bool>(t, "enabled").unwrap_or(true);

    if let Some(sub) = opt::<Table>(t, "items") {
        let children = build_items(app, sub)?;
        let refs: Vec<&dyn IsMenuItem<Wry>> =
            children.iter().map(|b| &**b as &dyn IsMenuItem<Wry>).collect();
        let submenu = Submenu::with_items(app, label, enabled, &refs).map_err(rt_err)?;
        return Ok(Box::new(submenu));
    }

    let id: String = opt(t, "id").unwrap_or_else(|| label.clone());
    let accel: Option<String> = opt(t, "accelerator");
    if let Some(checked) = opt::<bool>(t, "checked") {
        let item = CheckMenuItem::with_id(app, id, label, enabled, checked, accel.as_deref())
            .map_err(rt_err)?;
        Ok(Box::new(item))
    } else {
        let item = MenuItem::with_id(app, id, label, enabled, accel.as_deref()).map_err(rt_err)?;
        Ok(Box::new(item))
    }
}

fn build_items(app: &AppHandle, items: Table) -> mlua::Result<Vec<Item>> {
    let mut out: Vec<Item> = Vec::new();
    for v in items.sequence_values::<Value>() {
        match v? {
            Value::String(s) => out.push(predefined(app, &s.to_str()?)?),
            Value::Table(t) => out.push(from_table(app, &t)?),
            _ => return Err(rt_err("menu items must be strings or tables")),
        }
    }
    Ok(out)
}

fn tray_event_table(lua: &Lua, ev: &TrayIconEvent) -> Option<Table> {
    let t = lua.create_table().ok()?;
    match ev {
        TrayIconEvent::Click { button, button_state, .. } => {
            t.set("type", "click").ok()?;
            t.set("button", format!("{button:?}").to_lowercase()).ok()?;
            t.set("state", format!("{button_state:?}").to_lowercase()).ok()?;
        }
        TrayIconEvent::DoubleClick { button, .. } => {
            t.set("type", "double_click").ok()?;
            t.set("button", format!("{button:?}").to_lowercase()).ok()?;
        }
        TrayIconEvent::Enter { .. } => t.set("type", "enter").ok()?,
        TrayIconEvent::Move { .. } => t.set("type", "move").ok()?,
        TrayIconEvent::Leave { .. } => t.set("type", "leave").ok()?,
        _ => return None,
    }
    Some(t)
}

pub fn install(app: &AppHandle, lua: &Lua, tauri: &Table) -> mlua::Result<()> {
    let menu_t = lua.create_table()?;
    {
        let a = app.clone();
        menu_t.set(
            "new",
            lua.create_function(move |_, items: Table| {
                let built = build_items(&a, items)?;
                let refs: Vec<&dyn IsMenuItem<Wry>> =
                    built.iter().map(|b| &**b as &dyn IsMenuItem<Wry>).collect();
                Ok(LuaMenu(Menu::with_items(&a, &refs).map_err(rt_err)?))
            })?,
        )?;
    }
    tauri.set("menu", menu_t)?;

    // tauri.tray.new({ tooltip = "...", menu = menu, on_click = fn, on_menu = fn })
    let tray_t = lua.create_table()?;
    {
        let a = app.clone();
        tray_t.set(
            "new",
            lua.create_function(move |lua, o: Option<Table>| {
                let o = match o {
                    Some(o) => o,
                    None => lua.create_table()?,
                };
                let mut b = TrayIconBuilder::new();
                if let Some(icon) = a.default_window_icon() {
                    b = b.icon(icon.clone());
                }
                if let Some(tip) = opt::<String>(&o, "tooltip") {
                    b = b.tooltip(tip);
                }
                if let Some(menu) = opt::<UserDataRef<LuaMenu>>(&o, "menu") {
                    b = b.menu(&menu.0);
                }
                if let Some(v) = opt::<bool>(&o, "show_menu_on_left_click") {
                    b = b.show_menu_on_left_click(v);
                }
                if let Some(f) = opt::<Function>(&o, "on_menu") {
                    let cb = Callback::new(lua, f);
                    b = b.on_menu_event(move |_app, event| {
                        let lua = cb.lua();
                        if let Ok(t) = lua.create_table() {
                            let _ = t.set("id", event.id().0.clone());
                            cb.fire(t);
                        }
                    });
                }
                if let Some(f) = opt::<Function>(&o, "on_click") {
                    let cb = Callback::new(lua, f);
                    b = b.on_tray_icon_event(move |_tray, event| {
                        let lua = cb.lua();
                        if let Some(t) = tray_event_table(&lua, &event) {
                            cb.fire(t);
                        }
                    });
                }
                Ok(LuaTray(b.build(&a).map_err(rt_err)?))
            })?,
        )?;
    }
    tauri.set("tray", tray_t)
}
