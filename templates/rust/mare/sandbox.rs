// Locks down the Lua environment. Dangerous standard-library pieces are removed;
// the same abilities are available (permission-checked) as tauri.io / tauri.os.
use std::io::Write;

use mlua::{Lua, LuaOptions, StdLib, Variadic};

pub fn new_lua() -> mlua::Result<Lua> {
    // No `io`, no `debug`, no FFI. `os` is loaded only so we can copy safe pieces out of it.
    let libs = StdLib::COROUTINE
        | StdLib::TABLE
        | StdLib::STRING
        | StdLib::UTF8
        | StdLib::MATH
        | StdLib::OS
        | StdLib::PACKAGE;
    let lua = Lua::new_with(libs, LuaOptions::default())?;

    // io: stdout only. Files go through tauri.io.
    let io = lua.create_table()?;
    io.set(
        "write",
        lua.create_function(|_, parts: Variadic<String>| {
            let mut out = std::io::stdout().lock();
            for p in parts.iter() {
                let _ = out.write_all(p.as_bytes());
            }
            let _ = out.flush();
            Ok(())
        })?,
    )?;
    lua.globals().set("io", io)?;

    lua.load(
        r#"
        -- os: keep only side-effect-free functions
        local o = os
        os = { time = o.time, date = o.date, clock = o.clock, difftime = o.difftime }

        -- no reading arbitrary files as code, no bytecode, no native modules
        dofile = nil
        loadfile = nil
        local _load = load
        load = function(chunk, name, _mode, ...) return _load(chunk, name, "t", ...) end

        package.loadlib = nil
        package.cpath = ""
        if package.searchers then
            package.searchers[4] = nil
            package.searchers[3] = nil
        end
        "#,
    )
    .set_name("mare:sandbox")
    .exec()?;

    Ok(lua)
}

/// Lua-side glue that is easier to write in Lua than in Rust.
pub fn prelude(lua: &Lua) -> mlua::Result<()> {
    lua.load(
        r#"
        local states = {}
        function tauri.manage(name, value) states[name] = value end
        function tauri.state(name) return states[name] end

        local function fmt(...)
            local n = select("#", ...)
            local parts = {}
            for i = 1, n do parts[i] = tostring((select(i, ...))) end
            return table.concat(parts, " ")
        end
        for _, level in ipairs({ "trace", "debug", "info", "warn", "error" }) do
            tauri.log[level] = function(...) tauri.log._write(level, fmt(...)) end
        end
        "#,
    )
    .set_name("mare:prelude")
    .exec()
}
