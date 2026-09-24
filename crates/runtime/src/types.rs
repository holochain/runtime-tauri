use holochain::conductor::api::{AppAuthenticationTokenIssued, AppInfo};
use holochain_types::websocket::AllowedOrigins;

/// An app websocket port with an authentication token
#[derive(Clone, Debug)]
pub struct AppAuth {
    pub authentication: AppAuthenticationTokenIssued,
    pub port: u16,
    /// The `Origin` values the interface on `port` accepts.
    pub allowed_origins: AllowedOrigins,
}

/// What [`crate::Runtime::install_app_if_missing`] did.
#[derive(Clone, Debug)]
pub enum AppInstallOutcome {
    /// The app was not installed, and now is.
    Installed(Box<AppInfo>),
    /// The app was already installed; nothing was changed.
    AlreadyInstalled,
}
