use holochain::conductor::config::{ConductorConfig, KeystoreConfig, NetworkConfig};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use url2::Url2;

pub const DEVICE_SEED_LAIR_TAG: &str = "device-seed";

#[derive(Clone, Debug)]
pub struct RuntimeConfig {
    /// Path where conductor data is stored
    pub data_root_path: PathBuf,

    /// Network config
    pub network: RuntimeNetworkConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct RuntimeNetworkConfig {
    /// URL of the bootstrap server
    pub bootstrap_url: Url2,

    /// URL of the iroh relay server
    pub relay_url: Url2,
}

impl Default for RuntimeNetworkConfig {
    fn default() -> Self {
        Self {
            bootstrap_url: Url2::parse("https://dev-test-bootstrap2.holochain.org"),
            relay_url: Url2::parse("https://use1-1.relay.n0.iroh-canary.iroh.link./"),
        }
    }
}

impl From<RuntimeNetworkConfig> for NetworkConfig {
    fn from(val: RuntimeNetworkConfig) -> NetworkConfig {
        NetworkConfig {
            bootstrap_url: val.bootstrap_url,
            relay_url: val.relay_url,
            ..NetworkConfig::default()
        }
    }
}

impl From<RuntimeConfig> for ConductorConfig {
    fn from(val: RuntimeConfig) -> Self {
        Self {
            data_root_path: Some(val.data_root_path.clone().into()),
            keystore: KeystoreConfig::LairServerInProc { lair_root: None },
            network: val.network.into(),
            ..Self::default()
        }
    }
}

/// The longest Unix socket path, in bytes, that lair can bind: `sun_path` is 108
/// bytes on Linux and Android and 104 on Apple platforms, including the
/// terminating NUL.
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub const MAX_SOCKET_PATH_BYTES: usize = 103;
/// The longest Unix socket path, in bytes, that lair can bind: `sun_path` is 108
/// bytes on Linux and Android and 104 on Apple platforms, including the
/// terminating NUL.
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub const MAX_SOCKET_PATH_BYTES: usize = 107;

/// Check that lair's socket under `data_root_path` fits the Unix socket path
/// limit.
///
/// Holochain 0.7's in-process keystore still binds a socket at
/// `<data_root>/socket`, and an over-long path otherwise fails deep inside lair
/// with `path must be shorter than SUN_LEN`. Remove this once holochain's
/// in-process keystore no longer binds a socket.
pub fn check_data_root_path(data_root_path: &std::path::Path) -> crate::RuntimeResult<()> {
    let socket = data_root_path.join("socket");
    let len = socket.as_os_str().len();
    if len > MAX_SOCKET_PATH_BYTES {
        return Err(crate::RuntimeError::DataRootPathTooLong {
            path: socket.display().to_string(),
            len,
            max: MAX_SOCKET_PATH_BYTES,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_root_path_within_the_socket_limit_is_accepted() {
        let path = std::path::Path::new("/home/someone/.cache/app/holochain-0");
        assert!(check_data_root_path(path).is_ok());
    }

    #[test]
    fn data_root_path_over_the_socket_limit_is_rejected() {
        // "/" + 100 bytes + "/socket" is 108 bytes, over both platform limits.
        let path = std::path::PathBuf::from(format!("/{}", "a".repeat(100)));
        match check_data_root_path(&path) {
            Err(crate::RuntimeError::DataRootPathTooLong { len, max, .. }) => {
                assert_eq!(len, 108);
                assert_eq!(max, MAX_SOCKET_PATH_BYTES);
            }
            other => panic!("expected DataRootPathTooLong, got {other:?}"),
        }
    }
}
