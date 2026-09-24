//! Network settings a user saves from the app's UI, applied on top of the
//! network config the app builds.
//!
//! The commands are plugin commands, invoked as `plugin:hc|get_user_network_config`
//! and so on, and Tauri's ACL decides which windows may call them; which ones
//! `hc:default` includes and why is in `permissions/default.toml`.
//!
//! The app tells the plugin where the file lives by managing its path:
//!
//! ```ignore
//! tauri::Builder::default()
//!     .manage(tauri_plugin_hc::UserNetworkConfigPath(paths.user_network_config.clone()))
//! ```

use crate::{Error, NetworkConfig, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Runtime};
use url2::Url2;

/// Saved bootstrap and relay URLs. A field left `None` keeps the app's value.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserNetworkConfig {
    pub bootstrap_url: Option<Url2>,
    pub relay_url: Option<Url2>,
}

impl UserNetworkConfig {
    /// The settings saved at `path`, or `None` if nothing has been saved.
    pub fn read(path: &Path) -> Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }
        let contents = std::fs::read_to_string(path).map_err(|e| file_error(path, e))?;
        let config = serde_json::from_str(&contents).map_err(|e| file_error(path, e))?;
        Ok(Some(config))
    }

    /// Save the settings to `path`, creating its directory if needed.
    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| file_error(path, e))?;
        }
        let contents = serde_json::to_string(self).map_err(|e| file_error(path, e))?;
        std::fs::write(path, contents).map_err(|e| file_error(path, e))
    }

    /// Override `network`'s URLs with the ones set here.
    pub fn apply_to(&self, network: &mut NetworkConfig) {
        if let Some(url) = &self.bootstrap_url {
            network.bootstrap_url = url.clone();
        }
        if let Some(url) = &self.relay_url {
            network.relay_url = url.clone();
        }
    }

    /// Apply the settings saved at `path`, if any, to `network`. A file that
    /// can't be read is logged and ignored, so a corrupt file can't stop the app
    /// from starting.
    pub fn apply_saved(path: &Path, network: &mut NetworkConfig) {
        match Self::read(path) {
            Ok(Some(saved)) => saved.apply_to(network),
            Ok(None) => {}
            Err(e) => log::warn!("Ignoring saved network settings: {e}"),
        }
    }
}

fn file_error(path: &Path, e: impl std::fmt::Display) -> Error {
    Error::UserNetworkConfig(format!("{}: {e}", path.display()))
}

/// Managed state naming the file the user network commands read and write.
pub struct UserNetworkConfigPath(pub PathBuf);

/// The managed [`UserNetworkConfigPath`], or a clean error when the app never
/// registered one (a bare `State` argument would panic instead).
fn config_path<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf> {
    app.try_state::<UserNetworkConfigPath>()
        .map(|path| path.0.clone())
        .ok_or(Error::UserNetworkConfigPathNotManaged)
}

/// The saved network settings, or `null` if none have been saved.
#[tauri::command]
pub(crate) fn get_user_network_config<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<UserNetworkConfig>> {
    UserNetworkConfig::read(&config_path(&app)?)
}

/// Holochain's default bootstrap and relay URLs, to prefill or reset the form.
#[tauri::command]
pub(crate) fn default_user_network_config() -> UserNetworkConfig {
    let defaults = NetworkConfig::default();
    UserNetworkConfig {
        bootstrap_url: Some(defaults.bootstrap_url),
        relay_url: Some(defaults.relay_url),
    }
}

/// Save new bootstrap and relay URLs and restart the app so the conductor boots
/// with them. Outside `hc:default`; see `permissions/default.toml`.
#[tauri::command]
pub(crate) fn set_user_network_config<R: Runtime>(
    app: AppHandle<R>,
    bootstrap_url: Url2,
    relay_url: Url2,
) -> Result<()> {
    UserNetworkConfig {
        bootstrap_url: Some(bootstrap_url),
        relay_url: Some(relay_url),
    }
    .write(&config_path(&app)?)?;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_settings_override_only_the_urls_they_set() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("user-network-config.json");
        assert_eq!(UserNetworkConfig::read(&path).unwrap(), None);

        let saved = UserNetworkConfig {
            bootstrap_url: Some(Url2::parse("https://bootstrap.example.org")),
            relay_url: None,
        };
        saved.write(&path).unwrap();
        assert_eq!(UserNetworkConfig::read(&path).unwrap(), Some(saved));

        let mut network = crate::dev_network_config("http://127.0.0.1:8888");
        UserNetworkConfig::apply_saved(&path, &mut network);
        assert_eq!(
            network.bootstrap_url.as_str(),
            "https://bootstrap.example.org/"
        );
        assert_eq!(network.relay_url.as_str(), "http://127.0.0.1:8888/");
    }

    #[test]
    fn unreadable_settings_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user-network-config.json");
        std::fs::write(&path, "not json").unwrap();

        let mut network = NetworkConfig::default();
        let before = network.bootstrap_url.clone();
        UserNetworkConfig::apply_saved(&path, &mut network);
        assert_eq!(network.bootstrap_url, before);
    }

    #[test]
    fn commands_fail_cleanly_when_no_path_is_managed() {
        use tauri::test::{mock_builder, mock_context, noop_assets};
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("mock app builds");

        let result = get_user_network_config(app.handle().clone());
        assert!(
            matches!(result, Err(Error::UserNetworkConfigPathNotManaged)),
            "expected a managed-state error, got {result:?}"
        );
    }

    #[test]
    fn get_reads_the_managed_path() {
        use tauri::test::{mock_builder, mock_context, noop_assets};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user-network-config.json");
        let app = mock_builder()
            .manage(UserNetworkConfigPath(path.clone()))
            .build(mock_context(noop_assets()))
            .expect("mock app builds");

        assert_eq!(get_user_network_config(app.handle().clone()).unwrap(), None);
        let saved = UserNetworkConfig {
            bootstrap_url: Some(Url2::parse("https://bootstrap.example.org")),
            relay_url: None,
        };
        saved.write(&path).unwrap();
        assert_eq!(
            get_user_network_config(app.handle().clone()).unwrap(),
            Some(saved)
        );
    }

    #[test]
    fn file_format_matches_what_the_ui_sends() {
        let json = r#"{"bootstrapUrl":"https://b.example.org/","relayUrl":null}"#;
        let config: UserNetworkConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            config.bootstrap_url.unwrap().as_str(),
            "https://b.example.org/"
        );
        assert_eq!(config.relay_url, None);
    }
}
