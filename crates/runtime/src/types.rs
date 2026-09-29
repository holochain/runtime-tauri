use holochain::conductor::api::AppInfo;

/// What [`crate::Runtime::install_app_if_missing`] did.
#[derive(Clone, Debug)]
pub enum AppInstallOutcome {
    /// The app was not installed, and now is.
    Installed(Box<AppInfo>),
    /// The app was already installed; nothing was changed.
    AlreadyInstalled,
}
