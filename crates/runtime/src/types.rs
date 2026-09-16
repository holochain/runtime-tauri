use holochain::conductor::api::{AppAuthenticationTokenIssued, AppInfo};

/// An app websocket port with an authentication token
#[derive(Clone, Debug)]
pub struct AppAuth {
    pub authentication: AppAuthenticationTokenIssued,
    pub port: u16,
}

/// What [`crate::Runtime::install_app_if_missing`] did.
#[derive(Clone, Debug)]
pub enum AppInstallOutcome {
    /// The app was not installed, and now is.
    Installed(Box<AppInfo>),
    /// The app was already installed; nothing was changed.
    AlreadyInstalled,
}
