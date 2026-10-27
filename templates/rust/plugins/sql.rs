// tauri.sql: SQLite databases (via rusqlite), stored under the file permissions in mare.json.
//   local db = tauri.sql.open("$APPDATA/app.db")
//   db:execute("CREATE TABLE IF NOT EXISTS notes (id INTEGER PRIMARY KEY, text TEXT)")
//   db:execute("INSERT INTO notes (text) VALUES (?)", { "hello" })
//   for _, row in ipairs(db:query("SELECT * FROM notes")) do print(row.id, row.text) end
use std::sync::{Arc, Mutex};

use mlua::{Lua, Table, UserData, UserDataMethods, Value};
use rusqlite::{params_from_iter, types::Value as Sql, types::ValueRef, Connection};
use tauri::AppHandle;

use crate::mare::{
    paths::{self, Access},
    rt_err,
};

struct LuaDb(Arc<Mutex<Connection>>);

fn to_params(params: Option<Table>) -> mlua::Result<Vec<Sql>> {
    let mut out = Vec::new();
    if let Some(t) = params {
        for v in t.sequence_values::<Value>() {
            out.push(match v? {
                Value::Nil => Sql::Null,
                Value::Boolean(b) => Sql::Integer(b as i64),
                Value::Integer(i) => Sql::Integer(i),
                Value::Number(n) => Sql::Real(n),
                Value::String(s) => match s.to_str() {
                    Ok(text) => Sql::Text(text.to_string()),
                    Err(_) => Sql::Blob(s.as_bytes().to_vec()),
                },
                _ => return Err(rt_err("unsupported SQL parameter type")),
            });
        }
    }
    Ok(out)
}

impl UserData for LuaDb {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        // -> { changes, last_insert_id }
        m.add_method("execute", |lua, this, (sql, params): (String, Option<Table>)| {
            let conn = this.0.lock().unwrap();
            let changes = conn.execute(&sql, params_from_iter(to_params(params)?)).map_err(rt_err)?;
            let t = lua.create_table()?;
            t.set("changes", changes)?;
            t.set("last_insert_id", conn.last_insert_rowid())?;
            Ok(t)
        });

        m.add_method("exec_batch", |_, this, sql: String| {
            this.0.lock().unwrap().execute_batch(&sql).map_err(rt_err)
        });

        // -> array of row tables
        m.add_method("query", |lua, this, (sql, params): (String, Option<Table>)| {
            let conn = this.0.lock().unwrap();
            let mut stmt = conn.prepare(&sql).map_err(rt_err)?;
            let cols: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
            let mut rows = stmt.query(params_from_iter(to_params(params)?)).map_err(rt_err)?;
            let mut out = Vec::new();
            while let Some(row) = rows.next().map_err(rt_err)? {
                let t = lua.create_table()?;
                for (i, name) in cols.iter().enumerate() {
                    let v = match row.get_ref(i).map_err(rt_err)? {
                        ValueRef::Null => Value::Nil,
                        ValueRef::Integer(n) => Value::Integer(n),
                        ValueRef::Real(f) => Value::Number(f),
                        ValueRef::Text(b) | ValueRef::Blob(b) => Value::String(lua.create_string(b)?),
                    };
                    t.set(name.as_str(), v)?;
                }
                out.push(t);
            }
            Ok(out)
        });
    }
}

pub fn install(app: &AppHandle, lua: &Lua, tauri: &Table) -> mlua::Result<()> {
    let t = lua.create_table()?;
    let a = app.clone();
    t.set(
        "open",
        lua.create_function(move |_, path: String| {
            let p = paths::check(&a, &path, Access::Write).map_err(rt_err)?;
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).map_err(rt_err)?;
            }
            let conn = Connection::open(p).map_err(rt_err)?;
            Ok(LuaDb(Arc::new(Mutex::new(conn))))
        })?,
    )?;
    tauri.set("sql", t)
}
