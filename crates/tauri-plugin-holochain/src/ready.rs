//! Running app startup work once the conductor is up.

use crate::{HolochainExt, EVENT_READY};
use std::future::Future;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Listener, Runtime};

/// Run `f` once the conductor is ready, on Tauri's async runtime.
///
/// Use it from the app's `.setup()` for work that needs the conductor, such as
/// installing the hApp and opening the main window. Unlike listening for
/// [`EVENT_READY`] directly, `f` still runs if the conductor came up before this
/// was called, and it runs exactly once. An error returned by `f` is logged; to
/// show it to the user or exit, handle it inside `f`.
///
/// This does not report a failed boot: listen for [`crate::EVENT_SETUP_FAILED`]
/// for that.
pub fn on_ready<R, F, Fut, E>(app: &AppHandle<R>, f: F)
where
    R: Runtime,
    F: FnOnce(AppHandle<R>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), E>> + Send + 'static,
    E: std::fmt::Display,
{
    let pending = Arc::new(Mutex::new(Some(f)));

    // Listen first, then check: the plugin marks the runtime ready before it
    // emits the event, so whichever comes second finds `pending` already taken.
    let listener_app = app.clone();
    let listener_pending = pending.clone();
    app.once(EVENT_READY, move |_| {
        run_pending(&listener_app, &listener_pending)
    });

    let ready = app
        .holochain()
        .and_then(|plugin| plugin.try_runtime())
        .is_ok();
    if ready {
        run_pending(app, &pending);
    }
}

fn run_pending<R, F, Fut, E>(app: &AppHandle<R>, pending: &Mutex<Option<F>>)
where
    R: Runtime,
    F: FnOnce(AppHandle<R>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<(), E>> + Send + 'static,
    E: std::fmt::Display,
{
    let Some(f) = pending.lock().unwrap().take() else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = f(app).await {
            log::error!("Startup work after the conductor became ready failed: {e}");
        }
    });
}
