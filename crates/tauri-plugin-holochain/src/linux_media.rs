//! Camera and microphone access for the Linux webview.
//!
//! WebKitGTK only lets a page use `getUserMedia` if the embedder answers the
//! webview's `permission-request` signal; the default handler denies, and
//! neither wry nor Tauri connects one (they do on Android). So a hApp UI that
//! opens the camera, e.g. to scan a QR joining code, gets `NotAllowedError` on
//! Linux and works everywhere else. This grants media capture to the app's own
//! UI: a request is allowed only while the page making it is on the origin the
//! webview first loaded (see `origin.rs`), so a webview showing anything else
//! is denied. Plugin-built windows cannot leave that origin anyway; this covers
//! windows the app builds itself. The user still gets no OS-level prompt,
//! matching how the same UI behaves in the other webviews.
//!
//! The devices must also be visible to WebKit's GStreamer; the runtime-tauri
//! dev shell provides the capture plugins (see `GST_PLUGIN_PATH_1_0` in
//! flake.nix).

use crate::origin::same_origin;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{Runtime, Url, Webview};

/// Allow user-media (camera/microphone) permission requests on `webview` while
/// the page asking is on the origin recorded for it in `origins`. Other
/// permission requests keep WebKit's default handling.
pub(crate) fn allow_media_capture<R: Runtime>(
    webview: &Webview<R>,
    origins: Arc<Mutex<HashMap<String, Url>>>,
) {
    let label = webview.label().to_string();
    let result = webview.with_webview(move |platform| {
        use webkit2gtk::glib::prelude::*;
        use webkit2gtk::{
            PermissionRequestExt, SettingsExt, UserMediaPermissionRequest, WebViewExt,
        };

        let wv = platform.inner();
        // Off by default in older WebKitGTK; without it navigator.mediaDevices
        // is absent and no request is ever made.
        if let Some(settings) = wv.settings() {
            settings.set_enable_media_stream(true);
        }
        wv.connect_permission_request(move |wv, request| {
            if !request.is::<UserMediaPermissionRequest>() {
                // Let WebKit's default handler decide (it denies).
                return false;
            }
            let page = wv.uri().and_then(|uri| Url::parse(&uri).ok());
            let origin = origins.lock().unwrap().get(&label).cloned();
            match (&page, &origin) {
                (Some(page), Some(origin)) if same_origin(page, origin) => {
                    log::debug!("granting user-media permission to webview {label}");
                    request.allow();
                }
                _ => {
                    log::warn!(
                        "denying user-media permission to webview {label}: page {} is not on its origin {}",
                        page.map(|p| p.to_string()).unwrap_or_else(|| "<unknown>".into()),
                        origin.map(|o| o.to_string()).unwrap_or_else(|| "<none recorded>".into())
                    );
                    request.deny();
                }
            }
            true
        });
    });
    if let Err(e) = result {
        log::warn!("could not install media permission handler on webview: {e}");
    }
}
