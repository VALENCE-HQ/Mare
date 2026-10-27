<div align=center>
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/logo.png">
  <img width="120" height="134" alt="Mare" src="assets/logo.png" />
</picture>

---
# Mare
</div>

A scaffolder for desktop apps with a [Tauri](https://tauri.app) shell and a **Lua** backend.

Tauri normally wants you to write the backend in Rust. Mare generates a regular Tauri 2 project, then swaps
the Rust side for a small managed bootstrapper that runs your Lua instead. You write your backend in
`lua/main.lua`, and the frontend calls it like any other Tauri command:

```lua
-- lua/main.lua
tauri.command("greet", function(ctx, args)
  return "Hello, " .. args.name .. "!"
end)
```

```js
// frontend
const message = await invoke("greet", { name: "World" });
```

You still need Rust installed, because Tauri compiles it once. But you don't write any, and changing your
backend doesn't mean rebuilding it.

> **Unofficial.** Mare isn't affiliated with or endorsed by the Tauri project. It builds on top of Tauri and
> runs the official `create-tauri-app` to make the base project.

**Status:** early and experimental. It has been developed and tested on Linux; Windows and macOS haven't been
tried yet. The core (commands, windows, events, files) is more tested than the optional plugins, and the API
will change.

## What you need

- [Node.js](https://nodejs.org) 18 or newer
- [Rust](https://www.rust-lang.org/tools/install) (`cargo` should run)
- Tauri's system libraries and a C compiler (Mare's embedded Lua is built from source)

On Linux, for example:

```bash
sudo apt install build-essential libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev curl wget file          # Debian / Ubuntu
sudo pacman -S base-devel webkit2gtk-4.1 xdotool libappindicator-gtk3 \
  librsvg openssl curl wget file                                    # Arch
sudo dnf install webkit2gtk4.1-devel openssl-devel libxdo-devel \
  libappindicator-gtk3-devel librsvg2-devel curl wget file          # Fedora
```

Windows and macOS need Visual Studio Build Tools and Xcode Command Line Tools respectively. Tauri keeps the
current list on its [prerequisites page](https://v2.tauri.app/start/prerequisites/).

## Installing

```bash
npm install -g create-mare-app
```

Or skip the install and run it once with `npm create mare-app@latest`. To use a specific build, download the
`.tgz` from the [Releases](../../releases) page and run `npm install -g ./create-mare-app-<version>.tgz`.

## Making an app

```bash
mare my-app      # asks for a frontend, package manager and plugins
cd my-app
mare dev         # opens the app
```

The first build is slow because Rust compiles. After that it's quick, and edits to your Lua only need an app
restart, not a rebuild.

Other commands:

```bash
mare sync            # add or update Mare in an existing Tauri project (never touches lua/)
mare add all         # enable every plugin, or name them: mare add dialog store
mare remove sql      # disable a plugin
mare plugins         # list plugins
mare build           # release build
```

## What you can do from Lua

Mare mirrors Tauri's Rust API, so names match and are in `snake_case`: commands, startup hooks, background
tasks, windows, events, menus, a tray icon, custom URI protocols, channels, logging, plus scoped file access
(`tauri.io`) and system access (`tauri.os`).

Plugins are opt-in: `dialog`, `notification`, `opener`, `clipboard`, `store`, `http`, `global-shortcut`,
`autostart`, `updater` and `sql` (SQLite).

Every function has a description and types in `.mare/types/mare.lua`, which your editor shows as autocomplete
and hover help. Install the **Lua** extension (`sumneko.lua`) in VS Code. Neovim and Zed work with the Lua
language server too.

## Is Lua allowed to do anything?

No. Lua runs in a sandbox. Harmless parts of `os` and `io` stay (`os.time`, `os.date`, `io.write`, ...), and
the powerful ones moved to `tauri.io` and `tauri.os`, where they are permission-checked. By default your code
can only touch the app's own data, config, cache, log and temp folders, plus files the user picks in a dialog.
You grant more in `mare.json`, which is compiled into the app so it can't be changed after you ship.

Details and the full list of what replaced what are in [docs/SANDBOX.md](docs/SANDBOX.md).

## What it connects to

Mare collects nothing and has no accounts or telemetry. While creating or syncing a project it talks to:

- the npm registry, to run `create-tauri-app` and install your frontend dependencies
- crates.io, to add Rust dependencies (`mlua`, `tokio`, the plugins you picked)

Apps you build only use the network if you use it, for example through the `http` plugin.

## Building from source

```bash
git clone https://github.com/VALENCE-HQ/Mare
cd Mare
npm install
node index.js help
```

To try your changes, run `node index.js my-test-app`, or run `node index.js sync` inside an existing project.
See [CONTRIBUTING.md](CONTRIBUTING.md) and [docs/HACKING.md](docs/HACKING.md) for how the code is laid out.

## What it changes in your project

`Mare` adds or edits these, and nothing else:

- `lua/`: your backend (created once, never overwritten)
- `mare.json`: plugins, permissions and custom protocols (created once)
- `.mare/types/` and `.luarc.json`: editor autocomplete
- `src-tauri/src/mare/` and `src-tauri/src/lib.rs`: the managed Rust code (rewritten by `mare sync`)
- `src-tauri/Cargo.toml` and `tauri.conf.json`: dependencies, and bundling your `lua/` folder into releases

There's no eject command yet. To go back to plain Tauri, restore `lib.rs` from version control and delete
`src-tauri/src/mare/`.

At runtime, your app writes `app.log` to its log folder (`tauri.log.info(...)` goes there).

## AI Disclosure

Mare is developed and maintained by me, with AI assistance from Claude (by Anthropic).

## License

[Mozilla Public License 2.0](LICENSE).

Files Mare generates into your project (`src-tauri/src/mare/` and `src-tauri/src/lib.rs`) are covered by the
same license. Your own code is yours.
