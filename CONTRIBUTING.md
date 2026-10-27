# Contributing

Thanks for helping out. Mare is early, so bug reports and small fixes are the most useful things right now.

## Reporting a bug

Open an issue with:

- your OS, Node version (`node -v`) and Rust version (`rustc -V`)
- the command you ran
- the **first** error from the output. A single bad line in Rust usually causes a long list of follow-up
  errors, and only the first one matters.

## Working on the code

```bash
git clone https://github.com/YOUR_USERNAME/mare
cd mare
npm install
```

Try changes against a scratch project:

```bash
node index.js my-test-app      # create one
cd my-test-app
node ../index.js sync          # re-apply your changes to it
mare dev                       # or: npm run tauri dev
```

Before opening a pull request, run `node --check index.js`. CI does this on Node 18, 20 and 22.

## Guidelines

- The Rust under `templates/rust/` is generated into every user's project, so it must compile on its own.
  Test it in a real project, not just by reading it.
- If you change the Lua API, update `templates/types/mare.lua` in the same pull request. That file is what
  powers editor autocomplete.
- Keep the sandbox strict. New abilities that touch files, processes, the network or the environment must go
  through a permission check in `mare.json`.
- Small pull requests are easier to review than big ones.

## License

By contributing you agree that your changes are licensed under the [MPL-2.0](LICENSE).
