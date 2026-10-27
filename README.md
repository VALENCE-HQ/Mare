<div align=center>
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/Mare.svg">
  <img width="221" height="243" alt="Mare" src="assets/Mare.svg" />
</picture>

---
# Mare
</div>

A scaffolder for desktop apps with a [Tauri](https://tauri.app) shell and a Lua backend.

Tauri normally wants the backend written in Rust. Mare does the fiddly parts for you: it generates a regular
Tauri 2 project, swaps the Rust side for a small managed bootstrapper that runs your Lua, sets up editor
autocomplete, and adds the plugins you pick. You write your backend in `lua/main.lua`, and the frontend calls it
like any other Tauri command:

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

You still need Rust installed, because Tauri compiles it once. You just never write any, and changing your
backend doesn't mean rebuilding it.

> **Unofficial.** Mare isn't affiliated with or endorsed by the Tauri project. It builds on top of Tauri and
> runs the official `create-tauri-app` to make the base project.

**Status:** early. Developed and tested on Linux, on one machine so far. Windows and macOS haven't been tried.
The core (commands, windows, events, files) is better tested than the optional plugins, and the API will change.

## What you need

- [Node.js](https://nodejs.org) 18 or newer, and `git`
- [Rust](https://www.rust-lang.org/tools/install) (`cargo` should run)
- Tauri's system libraries and a C compiler (Mare's embedded Lua is built from source)

Missing something? On Linux, for example:

```bash
sudo apt install build-essential libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev curl wget file          # Debian / Ubuntu
sudo pacman -S base-devel webkit2gtk-4.1 xdotool libappindicator-gtk3 \
  librsvg openssl curl wget file                                    # Arch
sudo dnf install webkit2gtk4.1-devel openssl-devel libxdo-devel \
  libappindicator-gtk3-devel librsvg2-devel curl wget file          # Fedora
```

Windows needs the Visual Studio Build Tools and macOS needs the Xcode Command Line Tools. Tauri keeps the current
list on its [prerequisites page](https://v2.tauri.app/start/prerequisites/).

## Installing

Mare is a Node tool, so there's nothing to download by hand. Install it straight from this repo:

```bash
npm install -g github:VALENCE-HQ/Mare
mare --version
```

That gets the latest version. To pin a release, add its tag: `github:VALENCE-HQ/Mare#v0.3.4`. To update, run the
same command again. To uninstall, run `npm uninstall -g create-mare-app`.

Other package managers work the same way:

```bash
pnpm add -g github:VALENCE-HQ/Mare
yarn global add github:VALENCE-HQ/Mare
bun add -g github:VALENCE-HQ/Mare
```

Just want to try it once without installing anything? `npx github:VALENCE-HQ/Mare my-app`.

Prefer a file? Grab the `.tgz` from the [Releases](../../releases) page and run
`npm install -g ./create-mare-app-<version>.tgz`.

If `mare` isn't found afterwards, npm's global folder isn't on your `PATH`. Run `npm config get prefix` and add
its `bin` folder to your `PATH`.

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

Every function has a description and types in `.mare/types/mare.lua`, which your editor shows as autocomplete and
hover help. Install the **Lua** extension (`sumneko.lua`) in VS Code. Neovim and Zed work with the Lua language
server too.

## The sandbox

Lua runs in a sandbox, so it can't do just anything. The harmless parts of `os` and `io` stay (`os.time`,
`os.date`, `io.write`, ...), and the powerful ones moved to `tauri.io` and `tauri.os`, where they're
permission-checked. By default your code can only touch the app's own data, config, cache, log and temp folders,
plus files the user picks in a dialog. You grant more in `mare.json`, which is compiled into the app so it can't
be changed after you ship.

Details, and the full list of what replaced what, are in [docs/SANDBOX.md](docs/SANDBOX.md).

## What it connects to

Mare collects nothing and has no accounts or telemetry. While creating or syncing a project it talks to:

- the npm registry, to run `create-tauri-app` and install your frontend dependencies
- crates.io, to add Rust dependencies (`mlua`, `tokio`, and the plugins you picked)

Apps you build only use the network if you do, for example through the `http` plugin.

## Building from source

```bash
git clone https://github.com/VALENCE-HQ/Mare
cd Mare
npm install
node index.js help            # run the CLI from the repo
node index.js my-test-app     # make a scratch project with your changes
```

See [CONTRIBUTING.md](CONTRIBUTING.md) and [docs/HACKING.md](docs/HACKING.md) for how the code is laid out.

## What it changes in your project

Mare adds or edits these, and nothing else:

- `lua/`: your backend (created once, never overwritten)
- `mare.json`: plugins, permissions and custom protocols (created once)
- `.mare/types/` and `.luarc.json`: editor autocomplete
- `src-tauri/src/mare/` and `src-tauri/src/lib.rs`: the managed Rust code (rewritten by `mare sync`)
- `src-tauri/Cargo.toml` and `tauri.conf.json`: dependencies, and bundling your `lua/` folder into releases

There's no eject command yet. To go back to plain Tauri, restore `lib.rs` from version control and delete
`src-tauri/src/mare/`.

At runtime your app writes `app.log` to its log folder (`tauri.log.info(...)` goes there).

## AI Disclosure

Mare is developed and maintained by (Valence[https://github.com/VALENCE-HQ/]), with AI assistance from Claude (by Anthropic).

## License

[Mozilla Public License 2.0](LICENSE).

The files Mare generates into your project (`src-tauri/src/mare/` and `src-tauri/src/lib.rs`) are covered by the
same license. Your own code is yours.
