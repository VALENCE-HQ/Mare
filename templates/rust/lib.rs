// Managed by Mare (`mare sync` overwrites this). Your code lives in lua/.

mod mare;

use mare::{config, misc};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // Custom URI schemes listed in mare.json ("protocols") are routed to Lua
    for scheme in config::list_or(&["protocols"], &[]) {
        let name = scheme.clone();
        builder = builder.register_asynchronous_uri_scheme_protocol(
            scheme,
            move |ctx, request, responder| {
                let app = ctx.app_handle().clone();
                let name = name.clone();
                tauri::async_runtime::spawn(async move {
                    responder.respond(misc::handle_protocol(app, name, request).await);
                });
            },
        );
    }

    builder
        .invoke_handler(mare::invoke_handler)
        .setup(mare::setup)
        .build(tauri::generate_context!())
        .expect("error while building Mare app")
        .run(|app, event| misc::dispatch_run_event(app, &event));
}
