# The Lua sandbox

Mare runs your Lua in a locked-down environment. The idea: Lua code (including third-party Lua modules you
install) shouldn't be able to read your files, run programs or leak secrets unless you said it could.

## What changed from standard Lua

| Standard Lua | In Mare |
| --- | --- |
| `os.time`, `os.date`, `os.clock`, `os.difftime`, `io.write` | unchanged |
| `io.open`, `io.lines`, `io.read`, ... | `tauri.io.read_text`, `write_text`, `append_text`, `list_dir`, ... |
| `os.remove`, `os.rename` | `tauri.io.remove`, `tauri.io.rename` |
| `os.getenv` | `tauri.os.getenv` (only variables in `permissions.os.env`) |
| `os.execute`, `io.popen` | `tauri.os.execute` (only programs in `permissions.os.exec`) |
| `os.exit` | `tauri.os.exit` (quits through Tauri so cleanup runs) |
| `os.tmpname` | `tauri.os.tmpname` |
| `dofile`, `loadfile`, `package.loadlib`, native modules, `debug` | removed |
| `load` | text only (no bytecode) |

`require` only loads modules from your project's `lua/` folder.

## Paths

Anything in `tauri.io` takes a path with an optional variable at the start:

```lua
tauri.io.write_text("$APPDATA/notes/today.txt", "hello")
local text = tauri.io.read_text("$DOCUMENT/todo.txt")   -- only if you allowed $DOCUMENT
```

A path with no variable and no leading `/` is relative to `$APPDATA`.

Variables: `$APPDATA`, `$APPCONFIG`, `$APPCACHE`, `$APPLOG`, `$APPLOCALDATA`, `$RESOURCE`, `$TEMP`, `$HOME`,
`$DOCUMENT`, `$DOWNLOAD`, `$DESKTOP`, `$PICTURE`, `$AUDIO`, `$VIDEO`.

`..` and symlinks are resolved before checking, so a path can't escape an allowed folder.

## mare.json

```json
{
  "plugins": ["dialog", "notification", "opener"],
  "protocols": [],
  "permissions": {
    "fs": {
      "read":  ["$APPDATA", "$APPCONFIG", "$APPCACHE", "$APPLOG", "$TEMP", "$RESOURCE"],
      "write": ["$APPDATA", "$APPCONFIG", "$APPCACHE", "$APPLOG", "$TEMP"]
    },
    "os":  { "env": ["HOME", "USER", "USERNAME", "LANG"], "exec": [] },
    "net": ["*"]
  }
}
```

- **`fs.read` / `fs.write`**: folders (and everything inside them) Lua may read or write.
- **`os.env`**: environment variables `tauri.os.getenv` may return.
- **`os.exec`**: program names `tauri.os.execute` may run. Empty means none.
- **`net`**: hosts `tauri.http.fetch` may contact. `"*"` allows any; `"*.example.com"` allows subdomains.
- **`protocols`**: custom URI schemes you serve from Lua with `tauri.register_protocol`.
- **`plugins`**: managed by `mare add` / `mare remove`.

Run `mare sync` after editing. The file is compiled into the app, so permissions can't be changed after you
ship it.

## Runtime grants

Files the user picks in a `tauri.dialog` window, or drops onto an app window, become readable and writable
for the rest of the session even if they're outside the allowed folders. That mirrors how Tauri treats
user-selected files.

## What the sandbox does not do

It protects against Lua code reaching things through `os` and `io`. It is not a hardened security boundary
against a determined attacker, and your Rust plugins and the webview have their own permissions. Treat
third-party Lua modules with the same care as any dependency.
