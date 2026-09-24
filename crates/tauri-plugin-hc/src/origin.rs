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
//! anywhere else and to open new windows.

use tauri::Url;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
