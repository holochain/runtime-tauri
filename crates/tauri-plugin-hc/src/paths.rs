//! Where an app keeps its conductor data and its saved network settings.

use crate::{Error, Result};
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

/// How many dev instances can run at once, each with its own conductor data.
pub const MAX_DEV_INSTANCES: usize = 10;

/// File locations for an app built on this plugin, from [`app_paths`].
#[derive(Clone, Debug)]
pub struct AppPaths {
    /// Conductor and keystore data: pass it to [`crate::HolochainPluginConfig::new`].
    pub holochain_dir: PathBuf,
    /// The saved network settings file, for [`crate::UserNetworkConfig`] and the
    /// user network commands.
    pub user_network_config: PathBuf,
}

/// The data locations for the app `app_name` (usually the app id), choosing dev
/// or production layout with [`tauri::is_dev`].
///
/// - **Production:** `<user data dir>/<app_name>/holochain`, with the network
///   settings file beside it at `<user data dir>/<app_name>/user-network-config.json`.
/// - **Dev:** `<user cache dir>/<app_name>/holochain-<n>`, with the network
///   settings file inside it. Each running dev instance takes the first `n` whose
///   lock file is free, so `npm run start:tauri` with several agents gives each
///   its own conductor and settings. The lock is held until the process exits.
///
/// `author` is only used on Windows, where it is a path component; pass
/// `env!("CARGO_PKG_AUTHORS")` to match earlier releases of an app.
///
/// Call it once, before building the Tauri app: in dev every call takes another
/// instance slot.
pub fn app_paths(app_name: &'static str, author: &'static str) -> Result<AppPaths> {
    let data_type = if tauri::is_dev() {
        app_dirs2::AppDataType::UserCache
    } else {
        app_dirs2::AppDataType::UserData
    };
    let root = app_dirs2::app_root(
        data_type,
        &app_dirs2::AppInfo {
            name: app_name,
            author,
        },
    )
    .map_err(|e| Error::AppPaths(format!("could not find the app data directory: {e}")))?;

    let paths = if tauri::is_dev() {
        let holochain_dir = claim_dev_instance_dir(&root)?;
        AppPaths {
            user_network_config: holochain_dir.join(USER_NETWORK_CONFIG_FILE),
            holochain_dir,
        }
    } else {
        AppPaths {
            holochain_dir: root.join("holochain"),
            user_network_config: root.join(USER_NETWORK_CONFIG_FILE),
        }
    };
    holochain_conductor_runtime::check_data_root_path(&paths.holochain_dir)?;
    Ok(paths)
}

const USER_NETWORK_CONFIG_FILE: &str = "user-network-config.json";

/// Take the first `<root>/holochain-<n>` whose `.lock` no other process holds,
/// and keep holding it for the life of this process. Falls back to
/// `<root>/holochain` if all [`MAX_DEV_INSTANCES`] are taken.
fn claim_dev_instance_dir(root: &Path) -> Result<PathBuf> {
    for n in 0..MAX_DEV_INSTANCES {
        let dir = root.join(format!("holochain-{n}"));
        if let Some(lock) = try_lock_dir(&dir)? {
            // Released when the process exits.
            std::mem::forget(lock);
            return Ok(dir);
        }
    }
    Ok(root.join("holochain"))
}

/// Create `dir` and try to lock `dir/.lock` exclusively. `None` if another
/// handle already holds it.
fn try_lock_dir(dir: &Path) -> Result<Option<File>> {
    let io = |e: std::io::Error| Error::AppPaths(format!("{}: {e}", dir.display()));
    std::fs::create_dir_all(dir).map_err(io)?;
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(".lock"))
        .map_err(io)?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(e)) => Err(io(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_instances_take_the_next_free_slot() {
        let root = tempfile::tempdir().unwrap();

        let first = try_lock_dir(&root.path().join("holochain-0")).unwrap();
        assert!(first.is_some());
        // Held by `first`, so the next claim moves on to slot 1.
        assert_eq!(
            claim_dev_instance_dir(root.path()).unwrap(),
            root.path().join("holochain-1")
        );

        // Once slot 0 is released it is free again.
        drop(first);
        assert!(try_lock_dir(&root.path().join("holochain-0"))
            .unwrap()
            .is_some());
    }

    #[test]
    fn all_slots_taken_falls_back_to_a_shared_dir() {
        let root = tempfile::tempdir().unwrap();
        let _held: Vec<_> = (0..MAX_DEV_INSTANCES)
            .map(|n| try_lock_dir(&root.path().join(format!("holochain-{n}"))).unwrap())
            .collect();
        assert_eq!(
            claim_dev_instance_dir(root.path()).unwrap(),
            root.path().join("holochain")
        );
    }
}
