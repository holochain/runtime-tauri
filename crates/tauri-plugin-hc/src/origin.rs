//! The origin a webview is confined to: where its UI actually loaded from, and
//! nothing else.
//!
//! The origin is observed, not derived. Tauri resolves a `WebviewUrl` in ways
//! that depend on platform and build (a LAN `devUrl` is proxied through
//! `tauri://localhost` in mobile dev builds, custom schemes become
//! `http://<scheme>.<host>` on Windows and Android, `use_https_scheme` swaps the
//! scheme), and any hand-made copy of those rules is a blank window the first
//! time it is wrong. So the plugin records the origin of each webview's first
//! navigation or page load and locks to that. Every window from
//! [`crate::HolochainPlugin::main_window_builder`] then refuses to navigate
//! anywhere else and to open new windows, and the Linux camera/microphone grant
//! (`linux_media.rs`) is answered against the same record.
//!
//! The one place an origin still has to be predicted is the legacy app
//! websocket, whose `Origin` allow-list is fixed when the interface is attached,
//! before the page exists: see [`app_origin`].

use crate::{Error, Result};
use tauri::utils::config::FrontendDist;
use tauri::{AppHandle, Runtime, Url, WebviewUrl};

/// `url` reduced to its origin: scheme, host and explicit port, no path.
///
/// Kept as a `Url` so it can be compared with [`same_origin`] and printed.
pub fn origin_of(url: &Url) -> Url {
    Url::parse(&origin_header_value(url)).expect("scheme://host[:port] parses")
}

/// Whether `url` has the same scheme, host and port as `origin`.
///
/// Compared by hand rather than through [`Url::origin`], which treats every
/// non-special scheme (`tauri:`, `happ:`) as opaque and never equal to anything.
pub fn same_origin(url: &Url, origin: &Url) -> bool {
    url.scheme() == origin.scheme()
        && url.host_str() == origin.host_str()
        && url.port_or_known_default() == origin.port_or_known_default()
}

/// Whether a window locked to `origin` may navigate to `target`.
///
/// Same-origin URLs, and `blob:` URLs minted by that origin (`blob:<origin>/<id>`,
/// which is how a UI hands the user a file to save), are allowed. `data:` URLs
/// are not: they have no origin, and a top-level `data:` page is exactly the
/// look-alike the lock exists to refuse.
pub fn navigation_allowed(target: &Url, origin: &Url) -> bool {
    if same_origin(target, origin) {
        return true;
    }
    target.scheme() == "blob"
        && Url::parse(target.path())
            .map(|inner| same_origin(&inner, origin))
            .unwrap_or(false)
}

/// `origin` as a browser puts it in an `Origin` header: scheme, host and any
/// explicit port, no trailing slash.
pub(crate) fn origin_header_value(origin: &Url) -> String {
    let mut value = format!(
        "{}://{}",
        origin.scheme(),
        origin.host_str().unwrap_or_default()
    );
    if let Some(port) = origin.port() {
        value.push_str(&format!(":{port}"));
    }
    value
}

/// The origin Tauri will load `url` from, predicted from the app config the way
/// Tauri's own webview manager resolves it. Only the legacy app websocket needs
/// this, because its `Origin` allow-list must exist before the page does.
///
/// Mirrors Tauri 2.11: `devUrl` in a dev build (proxied through the app
/// protocol on mobile), `frontendDist` when it is a URL, otherwise the app
/// protocol; custom schemes are rewritten to `http://<scheme>.<host>` on
/// Windows and Android. It assumes the builder's default `use_https_scheme`
/// (off), which the plugin does not set; a consumer that turns it on gets a
/// websocket handshake refused on those platforms.
pub(crate) fn app_origin<R: Runtime>(app: &AppHandle<R>, url: &WebviewUrl) -> Result<Url> {
    let resolved = match url {
        WebviewUrl::External(url) => {
            let app_url = configured_app_url(app);
            if proxied_on_mobile(url) && app_url.make_relative(url).is_some() {
                tauri_protocol_url()
            } else {
                url.clone()
            }
        }
        WebviewUrl::CustomProtocol(url) => {
            if cfg!(any(windows, target_os = "android")) {
                let host = url.host_str().unwrap_or_default();
                Url::parse(&format!("http://{}.{host}", url.scheme()))
                    .map_err(|e| Error::UnsupportedWebviewUrl(format!("{url}: {e}")))?
            } else {
                url.clone()
            }
        }
        WebviewUrl::App(_) => {
            let app_url = configured_app_url(app);
            if proxied_on_mobile(&app_url) {
                tauri_protocol_url()
            } else {
                app_url
            }
        }
        other => return Err(Error::UnsupportedWebviewUrl(format!("{other:?}"))),
    };
    Ok(origin_of(&resolved))
}

/// Tauri's `get_app_url` with `use_https_scheme` off.
fn configured_app_url<R: Runtime>(app: &AppHandle<R>) -> Url {
    let config = app.config();
    let served_from = if tauri::is_dev() {
        config.build.dev_url.clone()
    } else {
        match &config.build.frontend_dist {
            Some(FrontendDist::Url(url)) => Some(url.clone()),
            _ => None,
        }
    };
    served_from.unwrap_or_else(tauri_protocol_url)
}

/// Tauri's `PROXY_DEV_SERVER && is_local_network_url`: a mobile dev build loads
/// a `localhost` or IP-address dev server through the app protocol instead.
fn proxied_on_mobile(url: &Url) -> bool {
    // `domain()` is `None` for IP-address hosts, which `host_str()` still has.
    let local_network = match (url.domain(), url.host_str()) {
        (Some(domain), _) => domain == "localhost",
        (None, Some(_ip)) => true,
        (None, None) => false,
    };
    tauri::is_dev() && cfg!(any(target_os = "android", target_os = "ios")) && local_network
}

/// Where Tauri serves embedded assets: `tauri://localhost`, or the wry
/// workaround `http://tauri.localhost` on Windows and Android.
fn tauri_protocol_url() -> Url {
    let url = if cfg!(any(windows, target_os = "android")) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    };
    Url::parse(url).expect("static app protocol url parses")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::{mock_builder, mock_context, noop_assets};

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn same_origin_matches_scheme_host_and_port_only() {
        let origin = url("tauri://localhost");
        assert!(same_origin(&url("tauri://localhost/index.html"), &origin));
        assert!(same_origin(
            &url("tauri://localhost/deep/path?x=1#y"),
            &origin
        ));
        assert!(!same_origin(&url("https://localhost/index.html"), &origin));
        assert!(!same_origin(
            &url("tauri://evil.example/index.html"),
            &origin
        ));
        assert!(!same_origin(&url("https://example.org/"), &origin));
        assert!(!same_origin(&url("about:blank"), &origin));

        let dev = url("http://localhost:1420");
        assert!(same_origin(&url("http://localhost:1420/src/main.ts"), &dev));
        assert!(!same_origin(&url("http://localhost:1421/"), &dev));
        assert!(!same_origin(&url("http://127.0.0.1:1420/"), &dev));
        // The default port is the same origin whether or not it is written.
        assert!(same_origin(
            &url("http://localhost:80/"),
            &url("http://localhost")
        ));
    }

    #[test]
    fn origin_of_keeps_scheme_host_and_explicit_port() {
        assert_eq!(
            origin_of(&url("tauri://localhost/index.html")).as_str(),
            "tauri://localhost"
        );
        assert_eq!(
            origin_of(&url("http://localhost:1420/index.html")).as_str(),
            "http://localhost:1420/"
        );
        assert_eq!(
            origin_header_value(&url("http://tauri.localhost/")),
            "http://tauri.localhost"
        );
        assert_eq!(
            origin_header_value(&url("https://ui.example.org:443/")),
            "https://ui.example.org"
        );
    }

    #[test]
    fn navigation_allows_same_origin_and_its_blobs_only() {
        let origin = url("tauri://localhost");
        assert!(navigation_allowed(
            &url("tauri://localhost/other.html"),
            &origin
        ));
        assert!(navigation_allowed(
            &url("blob:tauri://localhost/3f1c-4b2e"),
            &origin
        ));
        assert!(!navigation_allowed(
            &url("blob:https://evil.example/3f1c-4b2e"),
            &origin
        ));
        assert!(!navigation_allowed(&url("blob:nonsense"), &origin));
        assert!(!navigation_allowed(
            &url("data:text/html,<h1>look-alike</h1>"),
            &origin
        ));
        assert!(!navigation_allowed(&url("https://evil.example/"), &origin));
        assert!(!navigation_allowed(&url("about:blank"), &origin));

        let windows = url("http://tauri.localhost");
        assert!(navigation_allowed(
            &url("blob:http://tauri.localhost/3f1c"),
            &windows
        ));
    }

    #[test]
    fn app_url_resolves_to_the_app_protocol_when_nothing_else_is_configured() {
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("mock app builds");
        let handle = app.handle();
        assert_eq!(handle.config().build.dev_url, None);

        let origin = app_origin(handle, &WebviewUrl::App("index.html".into())).unwrap();
        assert_eq!(origin, tauri_protocol_url());
        assert!(same_origin(
            &tauri_protocol_url().join("index.html").unwrap(),
            &origin
        ));
    }

    #[test]
    fn external_urls_are_their_own_origin_on_desktop() {
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("mock app builds");
        let handle = app.handle();

        let external = url("https://ui.example.org/app/index.html");
        assert_eq!(
            app_origin(handle, &WebviewUrl::External(external)).unwrap(),
            url("https://ui.example.org")
        );
        let custom = url("happ://forum/index.html");
        let expected = if cfg!(any(windows, target_os = "android")) {
            url("http://happ.forum")
        } else {
            url("happ://forum")
        };
        assert_eq!(
            app_origin(handle, &WebviewUrl::CustomProtocol(custom)).unwrap(),
            expected
        );
    }
}
