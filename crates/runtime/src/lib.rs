//! A simple wrapper around the [`holochain`](https://docs.rs/holochain/latest/holochain/) crate, with functions to expose interacting with the [`holochain::Conductor`](https://docs.rs/holochain/latest/holochain/conductor/index.html) directly.
//!
//! It is intended to be a general-purpose wrapper for holochain runtime.
//!
//! It carries no host-framework or platform concerns: the Tauri integration
//! lives in `tauri-plugin-holochain`, which is its only consumer here.

mod runtime;
pub use runtime::*;

pub mod hc_auth;
pub use hc_auth::{HcAuthConfig, HcAuthStatus};

mod error;
pub use error::*;
pub use holochain::conductor::error::ConductorError;

mod config;
pub use config::*;

mod types;
pub use types::*;

