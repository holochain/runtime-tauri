//! Camera and microphone access for the Linux webview.
//!
//! WebKitGTK only lets a page use `getUserMedia` if the embedder answers the
//! webview's `permission-request` signal; the default handler denies, and
//! neither wry nor Tauri connects one (they do on Android). So a hApp UI that
//! opens the camera, e.g. to scan a QR joining code, gets `NotAllowedError` on
//! Linux and works everywhere else. This grants media capture for every
//! webview the plugin sees, which is the app's own bundled UI, not arbitrary
//! web content. The user still gets no OS-level prompt, matching how the same
//! UI behaves in the other webviews.
//!
//! The devices must also be visible to WebKit's GStreamer; the runtime-tauri
//! dev shell provides the capture plugins (see `GST_PLUGIN_PATH_1_0` in
//! flake.nix).

use tauri::{Runtime, Webview};

/// Allow user-media (camera/microphone) permission requests on `webview`.
/// Other permission requests keep WebKit's default handling.
pub(crate) fn allow_media_capture<R: Runtime>(webview: &Webview<R>) {
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
        wv.connect_permission_request(move |_, request| {
            if request.is::<UserMediaPermissionRequest>() {
                log::debug!("granting user-media permission to webview {label}");
                request.allow();
                true
            } else {
                // Let WebKit's default handler decide (it denies).
                false
            }
        });
    });
    if let Err(e) = result {
        log::warn!("could not install media permission handler on webview: {e}");
    }
}
