-- Mare: your backend lives here.
--
-- The frontend calls these like normal Tauri commands:
--   invoke("greet", { name: "World" })

---@param ctx mare.CommandContext
---@param args { name: string }
---@return string
tauri.command("greet", function(ctx, args)
  tauri.log.info("greeting", args.name, "from window", ctx.webview_label)
  return "Hello, " .. args.name .. "! You've been greeted from Lua!"
end)

-- Runs once at startup (like Builder::setup in Rust)
tauri.setup(function(app)
  tauri.log.info("Mare is running", app:package_info().version)
end)
