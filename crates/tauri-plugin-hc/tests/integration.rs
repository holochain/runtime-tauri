//! Integration tests (Approach A): prove the plugin boots a real Holochain
//! conductor *inside a Tauri app* and serves it both ways — the legacy app
//! websocket and the new in-process `app_request` IPC path.
//!
//! These drive the plugin as a real app would: build a Tauri app with the
//! plugin and wait for `holochain://ready`. They do not open a webview — the
//! full webview + `@holochain/client` path is covered by the desktop example
//! app (Approach B).

use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::time::Instant;

use holochain::conductor::api::CellInfo::Provisioned;
use holochain::conductor::api::{AppRequest, AppResponse, ProvisionedCell};
use holochain::prelude::{
    decode, encode, AppBundleSource, ExternIO, InstallAppPayload, ZomeCallParams,
};
use holochain_types::prelude::{AppStatus, Nonce256Bits, Record, Timestamp};
use tauri::test::{mock_builder, mock_context, noop_assets};
use tauri_plugin_hc::test_support::{build_app, wait_for_ready, BOOT_TIMEOUT};
use tauri_plugin_hc::{
    AppInstallOutcome, Error, HolochainExt, HolochainPluginConfig, NetworkConfig,
};
use tempfile::TempDir;
use uuid::Uuid;

use test_happ::{test_happ_bytes, ROLE_NAME, ZOME_NAME};

const APP_ID: &str = "test-app";

async fn install_and_enable_test_happ(
    runtime: &tauri_plugin_hc::Runtime,
) -> holochain::conductor::api::AppInfo {
    let app_info = runtime
        .install_app(InstallAppPayload {
            source: AppBundleSource::Bytes(test_happ_bytes().into()),
            agent_key: None,
            installed_app_id: Some(APP_ID.into()),
            network_seed: Some(Uuid::new_v4().to_string()),
            roles_settings: Some(HashMap::new()),
            ignore_genesis_failure: false,
            restore_from_dht: false,
        })
        .await
        .expect("install_app failed");
    runtime
        .enable_app(APP_ID.into())
        .await
        .expect("enable_app failed");
    app_info
}

#[test]
fn plugin_boots_conductor_in_tauri_app() {
    let tmp = TempDir::new().unwrap();
    let app = build_app(tmp.path());

    // Everything runs on Tauri's async runtime (where the plugin spawned the
    // conductor boot), avoiding cross-runtime issues.
    tauri::async_runtime::block_on(async move {
        wait_for_ready(&app).await;
        let runtime = app.holochain().unwrap().runtime();

        install_and_enable_test_happ(&runtime).await;

        // Attach an app interface — this is the websocket the legacy injection
        // wires a webview to. A real bound port proves the endpoint exists.
        let app_auth = runtime
            .ensure_app_websocket(APP_ID.into(), tauri_plugin_hc::AllowedOrigins::Any)
            .await
            .expect("ensure_app_websocket failed");
        assert!(app_auth.port > 0, "expected a bound app interface port");
        assert!(
            !app_auth.authentication.token.is_empty(),
            "expected a non-empty app auth token"
        );

        let apps = runtime.list_apps().await.expect("list_apps failed");
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].installed_app_id, APP_ID);
        assert_eq!(apps[0].status, AppStatus::Enabled);
    });
}

/// The `app_request` IPC path serves the App API in-process: a window bound to
/// an app can fetch app info and make a (signed) zome call, with no app
/// websocket attached. The bytes use holochain's `SerializedBytes` codec —
/// identical to what `@holochain/client`'s Tauri transport sends.
#[test]
fn app_request_serves_app_api_in_process() {
    let tmp = TempDir::new().unwrap();
    let app = build_app(tmp.path());

    tauri::async_runtime::block_on(async move {
        wait_for_ready(&app).await;
        let plugin = app.holochain().unwrap();
        let runtime = plugin.runtime();

        let app_info = install_and_enable_test_happ(&runtime).await;

        // main_window_builder does this in real use; bind a label directly here.
        plugin.bind_window("main", APP_ID.into());

        // AppInfo over the IPC codec.
        let resp_bytes = plugin
            .app_request_bytes("main", encode(&AppRequest::AppInfo).unwrap())
            .await
            .expect("app_request AppInfo failed");
        let resp: AppResponse = decode(&resp_bytes).unwrap();
        let AppResponse::AppInfo(Some(info)) = resp else {
            panic!("expected AppResponse::AppInfo(Some(_)), got {resp:?}");
        };
        assert_eq!(info.installed_app_id, APP_ID);

        // A signed CallZome over the IPC codec round-trips to an empty list.
        let Provisioned(ProvisionedCell { cell_id, .. }) =
            app_info.cell_info.get(ROLE_NAME).unwrap().first().unwrap()
        else {
            panic!("App Info has no CellId")
        };
        let signed = runtime
            .sign_zome_call(ZomeCallParams {
                provenance: cell_id.agent_pubkey().clone(),
                cell_id: cell_id.clone(),
                zome_name: ZOME_NAME.into(),
                fn_name: "get_test_entries".into(),
                cap_secret: None,
                payload: ExternIO::encode(()).unwrap(),
                nonce: Nonce256Bits::from([0; 32]),
                expires_at: Timestamp(Timestamp::now().as_micros() + 60_000_000),
            })
            .await
            .unwrap();
        let resp_bytes = plugin
            .app_request_bytes(
                "main",
                encode(&AppRequest::CallZome(Box::new(signed))).unwrap(),
            )
            .await
            .expect("app_request CallZome failed");
        let resp: AppResponse = decode(&resp_bytes).unwrap();
        let AppResponse::ZomeCalled(io) = resp else {
            panic!("expected AppResponse::ZomeCalled, got {resp:?}");
        };
        let entries: Vec<Record> = io.decode().unwrap();
        assert!(entries.is_empty());

        // A request from an unbound window is refused — a window can only reach
        // the app it was opened for.
        let unbound = plugin
            .app_request_bytes("not-bound", encode(&AppRequest::AppInfo).unwrap())
            .await;
        assert!(matches!(unbound, Err(Error::WindowNotBound)));
    });
}

/// `rebind_window` re-routes a window's `app_request` to a different app — and
/// unbinds it — in place, so a consumer can move one persistent window between
/// apps (and to/from an app-less dashboard) without recreating it.
#[test]
fn rebind_window_reroutes_app_request_in_place() {
    let tmp = TempDir::new().unwrap();
    let app = build_app(tmp.path());

    tauri::async_runtime::block_on(async move {
        wait_for_ready(&app).await;
        let plugin = app.holochain().unwrap();
        let runtime = plugin.runtime();

        // Two enabled apps to rebind between.
        install_and_enable_test_happ(&runtime).await;
        const APP_ID_2: &str = "test-app-2";
        runtime
            .install_app(InstallAppPayload {
                source: AppBundleSource::Bytes(test_happ_bytes().into()),
                agent_key: None,
                installed_app_id: Some(APP_ID_2.into()),
                network_seed: Some(Uuid::new_v4().to_string()),
                roles_settings: Some(HashMap::new()),
                ignore_genesis_failure: false,
                restore_from_dht: false,
            })
            .await
            .expect("install test-app-2 failed");
        runtime
            .enable_app(APP_ID_2.into())
            .await
            .expect("enable test-app-2 failed");

        // Bind to the first app, then rebind to the second — the route follows
        // the binding, with no recreate.
        plugin
            .rebind_window("main", Some(APP_ID.into()))
            .await
            .unwrap();
        let resp: AppResponse = decode(
            &plugin
                .app_request_bytes("main", encode(&AppRequest::AppInfo).unwrap())
                .await
                .expect("app_request AppInfo failed"),
        )
        .unwrap();
        assert!(
            matches!(&resp, AppResponse::AppInfo(Some(i)) if i.installed_app_id == APP_ID),
            "expected AppInfo for {APP_ID}, got {resp:?}"
        );

        plugin
            .rebind_window("main", Some(APP_ID_2.into()))
            .await
            .unwrap();
        let resp: AppResponse = decode(
            &plugin
                .app_request_bytes("main", encode(&AppRequest::AppInfo).unwrap())
                .await
                .expect("app_request AppInfo failed"),
        )
        .unwrap();
        assert!(
            matches!(&resp, AppResponse::AppInfo(Some(i)) if i.installed_app_id == APP_ID_2),
            "expected AppInfo for {APP_ID_2}, got {resp:?}"
        );

        // Unbind — the window can no longer reach any app.
        plugin.rebind_window("main", None).await.unwrap();
        let unbound = plugin
            .app_request_bytes("main", encode(&AppRequest::AppInfo).unwrap())
            .await;
        assert!(matches!(unbound, Err(Error::WindowNotBound)));
    });
}

/// A *failed* rebind must not change the window's routing. `init_deferred`
/// leaves the runtime `NotReady`, so `rebind_window`'s `spawn_signal_forwarder`
/// fails deterministically — the previous binding must survive (no half-rebind
/// where `app_request` points at an app whose signal stream never started).
#[test]
fn rebind_failed_spawn_keeps_prior_binding() {
    let tmp = TempDir::new().unwrap();
    let app = mock_builder()
        .plugin(tauri_plugin_hc::init_deferred(HolochainPluginConfig::new(
            tmp.path().to_path_buf(),
            NetworkConfig::default(),
        )))
        .build(mock_context(noop_assets()))
        .expect("failed to build mock tauri app");
    let plugin = app.holochain().unwrap();

    plugin.bind_window("main", APP_ID.into());
    assert_eq!(plugin.bound_app("main"), Some(APP_ID.to_string()));

    let result =
        tauri::async_runtime::block_on(plugin.rebind_window("main", Some("other-app".into())));
    assert!(
        matches!(result, Err(Error::NotReady)),
        "expected the rebind to fail with NotReady, got {result:?}"
    );
    assert_eq!(
        plugin.bound_app("main"),
        Some(APP_ID.to_string()),
        "a failed rebind must not flip the window's routing to the new app"
    );
}

/// Dropping a window clears its routing — the same `drop_window` path the
/// window-destroy handler (`plugin_builder`'s `on_event` → `RunEvent::WindowEvent`
/// `Destroyed`) runs to prune a closed window's maps and abort its forwarder. The
/// OS-window-destroy event itself only fires on a real window close, which the
/// mock test runtime doesn't drive, so this exercises the cleanup it calls.
#[test]
fn unbind_drops_window_routing() {
    let tmp = TempDir::new().unwrap();
    let app = mock_builder()
        .plugin(tauri_plugin_hc::init_deferred(HolochainPluginConfig::new(
            tmp.path().to_path_buf(),
            NetworkConfig::default(),
        )))
        .build(mock_context(noop_assets()))
        .expect("failed to build mock tauri app");
    let plugin = app.holochain().unwrap();

    plugin.bind_window("main", APP_ID.into());
    assert_eq!(plugin.bound_app("main"), Some(APP_ID.to_string()));

    tauri::async_runtime::block_on(plugin.rebind_window("main", None)).unwrap();
    assert_eq!(
        plugin.bound_app("main"),
        None,
        "dropping a window must clear its routing"
    );
}

/// The shipped JS bundle is built from `guest-js/` and injected verbatim by
/// `main_window_builder` (`include_str!` of this same file). No runtime test
/// exercises the injected JS, so a stale bundle silently ships the old rebound
/// listener — which read the whole event payload as the app id, with no seq
/// gate. Guard against that: the regenerated bundle reads the structured fields.
#[test]
fn shipped_bundle_matches_rebound_payload_shape() {
    let bundle = include_str!("../dist-js/holochain-env/index.min.js");
    assert!(
        bundle.contains("payload.app_id") && bundle.contains("payload.seq"),
        "dist-js/holochain-env/index.min.js is stale — run `npm run build` in crates/tauri-plugin-hc"
    );
}

/// Both injection modes must take the plugin identifier from the Rust side. A
/// literal name baked into the bundle would make the zome-call signer invoke a
/// plugin the ACL does not know — the legacy websocket path once hardcoded
/// `"holochain"`, which broke silently when the identifier became `hc`.
#[test]
fn shipped_bundle_does_not_hardcode_a_plugin_name() {
    let bundle = include_str!("../dist-js/holochain-env/index.min.js");
    for literal in ["\"holochain\"", "'holochain'", "\"hc\"", "'hc'"] {
        assert!(
            !bundle.contains(literal),
            "dist-js bundle hardcodes the plugin name {literal}; pass it in from PLUGIN_NAME"
        );
    }
}

/// A conductor that fails to boot has to say why: the plugin holds the setup
/// error, so a waiter fails on the cause at once instead of sitting out
/// `BOOT_TIMEOUT` and reporting only that nothing became ready.
#[test]
fn failed_boot_reports_its_cause_instead_of_timing_out() {
    let tmp = TempDir::new().unwrap();
    // A regular file cannot hold the conductor's data root, so lair fails to
    // spawn and the boot errors within moments of the app being built.
    let data_root = tmp.path().join("data-root");
    std::fs::write(&data_root, b"").unwrap();
    let app = build_app(&data_root);

    let started = Instant::now();
    let panic = std::panic::catch_unwind(AssertUnwindSafe(|| {
        tauri::async_runtime::block_on(wait_for_ready(&app));
    }))
    .expect_err("waiting on a conductor that cannot boot must fail the test");
    let elapsed = started.elapsed();

    let Err(Error::SetupFailed(cause)) = app.holochain().unwrap().try_runtime() else {
        panic!("a failed boot must be reported as SetupFailed, not NotReady");
    };
    let message = panic
        .downcast_ref::<String>()
        .expect("the panic message is formatted, so it is a String");
    assert!(
        message.contains(&cause),
        "the waiter must fail on the boot error, got: {message}"
    );
    assert!(
        elapsed < BOOT_TIMEOUT / 2,
        "the boot error must surface without waiting out {BOOT_TIMEOUT:?}, took {elapsed:?}"
    );
}

/// `on_ready` runs its work once the conductor is up, whether it was registered
/// while the boot was still in flight or after it had finished.
#[test]
fn on_ready_runs_before_and_after_boot() {
    let tmp = TempDir::new().unwrap();
    let app = build_app(tmp.path());

    let (early_tx, early_rx) = tokio::sync::oneshot::channel();
    tauri_plugin_hc::on_ready(app.handle(), move |handle| async move {
        let ready = handle.holochain().and_then(|p| p.try_runtime()).is_ok();
        early_tx.send(ready).unwrap();
        Ok::<_, Error>(())
    });

    tauri::async_runtime::block_on(async move {
        let registered_early_saw_runtime = tokio::time::timeout(BOOT_TIMEOUT, early_rx)
            .await
            .expect("on_ready registered before boot did not run")
            .unwrap();
        assert!(registered_early_saw_runtime);

        let (late_tx, late_rx) = tokio::sync::oneshot::channel();
        tauri_plugin_hc::on_ready(app.handle(), move |_| async move {
            late_tx.send(()).unwrap();
            Ok::<_, Error>(())
        });
        tokio::time::timeout(std::time::Duration::from_secs(5), late_rx)
            .await
            .expect("on_ready registered after boot did not run")
            .unwrap();
    });
}

/// `install_app_if_missing` installs and enables on first run and leaves the app
/// alone afterwards, including when it has been disabled.
#[test]
fn install_app_if_missing_installs_once() {
    let tmp = TempDir::new().unwrap();
    let app = build_app(tmp.path());

    tauri::async_runtime::block_on(async move {
        wait_for_ready(&app).await;
        let runtime = app.holochain().unwrap().runtime();
        let payload = || InstallAppPayload {
            source: AppBundleSource::Bytes(test_happ_bytes().into()),
            agent_key: None,
            installed_app_id: Some(APP_ID.into()),
            network_seed: Some(Uuid::new_v4().to_string()),
            roles_settings: Some(HashMap::new()),
            ignore_genesis_failure: false,
            restore_from_dht: false,
        };

        let first = runtime
            .install_app_if_missing(payload(), true)
            .await
            .expect("first install failed");
        assert!(matches!(first, AppInstallOutcome::Installed(_)));
        let apps = runtime.list_apps().await.unwrap();
        assert_eq!(apps[0].status, AppStatus::Enabled);

        runtime.disable_app(APP_ID.into()).await.unwrap();
        let second = runtime
            .install_app_if_missing(payload(), true)
            .await
            .expect("second call failed");
        assert!(matches!(second, AppInstallOutcome::AlreadyInstalled));
        let apps = runtime.list_apps().await.unwrap();
        assert_eq!(apps.len(), 1);
        assert!(matches!(apps[0].status, AppStatus::Disabled(_)));
    });
}
