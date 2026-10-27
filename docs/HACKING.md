# Hacking on Mare

How the repo is laid out and how the pieces fit together.

## The two halves

**The CLI** (`index.js`) runs on the developer's machine. It creates a Tauri project, copies Mare's files into
it, edits a few config files and runs `cargo add`.

**The generated Rust** (`templates/rust/`) is what runs inside the finished app. It embeds a Lua runtime,
runs `lua/main.lua`, and routes calls from the frontend to Lua.

## Layout

```
index.js                     the CLI (create, sync, add, remove, plugins, dev, build)
templates/
  mare.json                  default plugins and permissions for new projects
  main.lua                   starter backend
  luarc.json                 language server settings
  types/mare.lua             editor types for the whole Lua API
  rust/
    lib.rs                   entry point; replaces src-tauri/src/lib.rs
    mare/                    core: always copied (wiped and rewritten on sync)
      mod.rs                 runtime, the `tauri` table, command routing, startup
      sandbox.rs             removes dangerous Lua functions
      paths.rs  io.rs  os.rs path variables, permission checks, tauri.io / tauri.os
      app.rs  window.rs      `app` and window objects
      menu.rs  misc.rs       menus and tray; logging, json, channels, protocols, run events
      config.rs              reads mare.json (embedded at compile time)
    plugins/                 optional: only enabled ones are copied in
```

## How a frontend call reaches Lua

1. The frontend calls `invoke("greet", { name })`.
2. `invoke_handler` in `mare/mod.rs` sees the command name. If Lua registered it with `tauri.command`, the
   call is handled; otherwise Tauri handles it as usual.
3. `call_lua` builds a `ctx` table (`app`, `window`, `state`, ...), converts the JSON args to Lua, and runs
   the function. The return value becomes the resolved promise.

## Permissions

`mare.json` is read with `include_str!`, so it is baked into the binary. `paths::check` is the single place
file access is decided. Anything new that touches the outside world should call into `config` the same way.

## Adding a plugin

1. Add `templates/rust/plugins/<name>.rs` with a `pub fn install(app, lua, tauri)` that registers the
   plugin (if it needs it) and sets `tauri.<name>`. File names use underscores (`global_shortcut.rs`).
2. Add an entry to `PLUGINS` in `index.js` with the crate name.
3. Add the Lua API to `templates/types/mare.lua`.
4. Add it to the plugin lists in `README.md`.

## Adding to the core API

Add the method in the matching file under `templates/rust/mare/`, then describe it in
`templates/types/mare.lua`. Window methods live in `window.rs`, app methods in `app.rs`.

## Testing

There's no Rust test suite yet. The workflow is: make a scratch project with `node index.js scratch`, run
`mare dev`, and exercise the API from `lua/main.lua`. CI only checks that the CLI loads and packs.

## Releasing

Releases are made from your machine with `scripts/release.mjs`. You need `git` and the
[GitHub CLI](https://cli.github.com) (`gh auth login` once).

```bash
npm run release:dry             # run every check, push and publish nothing
npm run release -- patch        # bump the version, then release (patch | minor | major | prerelease | 1.2.3)
npm run release                 # release the version already in package.json
npm run release -- patch --npm  # also publish to npm (needs `npm login`)
```

The script:

1. checks you're on a branch with a clean tree, and that the tag matches `package.json`
2. runs `npm install` and sanity-checks the CLI
3. runs `npm pack`
4. pushes the branch and tag, then creates a GitHub Release with the `.tgz` attached and auto-generated
   notes. Versions containing a `-` (like `0.4.0-beta.1`) are marked as prereleases.

`.github/workflows/release.yml` does the same thing in CI, but only when you run it by hand from the Actions tab.
It is not triggered by tags, so it won't clash with the script.
