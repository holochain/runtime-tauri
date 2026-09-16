//! Network config for a local development network: every agent uses one
//! `kitsune2-bootstrap-srv` on the developer's machine as both its bootstrap
//! server and its iroh relay, so dev never reaches the public servers.

use crate::NetworkConfig;

/// Gossip `initiateBurstFactor` used on dev networks, in place of kitsune2's
/// default of 3.
///
/// A peer accepts `initiateBurstFactor x initiateBurstWindowCount` gossip rounds
/// from any one peer per `initiateIntervalMs x initiateBurstWindowCount`, which
/// with the defaults is 15 rounds per 10 minutes, and drops the rest without
/// replying. A node whose storage arc is below its target (as after every
/// conductor start) initiates about once a second, so on a two- or three-agent
/// network it can reach that limit with a peer within seconds. Between a desktop
/// and a phone agent on the default we saw the phone drop the desktop's rounds
/// every ~20 s and new data take minutes to arrive; with 100 (500 rounds per 10
/// minutes) there were no drops and a fresh phone agent synced in about 90 s. Why
/// the desktop's arc stayed below its target in that run is not yet established.
///
/// It is a per-peer flood guard, so this is set for dev only.
pub const DEV_INITIATE_BURST_FACTOR: u32 = 100;

/// A [`NetworkConfig`] that uses `url` (e.g. `http://192.168.1.20:8888`) as both
/// the bootstrap server and the iroh relay, as `kitsune2-bootstrap-srv` serves
/// both on one address.
///
/// Beyond the URLs it sets, through `advanced`:
/// - `irohTransport.relayAllowPlainText`: the local server's relay is plain
///   `http://`, which the iroh transport otherwise rejects.
/// - `k2Gossip.initiateBurstFactor` to [`DEV_INITIATE_BURST_FACTOR`].
///
/// Every agent on a dev network must be given the same `url`, not just one that
/// reaches the server: peers advertise their relay URL, and kitsune2 fails the
/// connection preflight for a peer on a relay it doesn't know. Use
/// [`dev_network_url!`](crate::dev_network_url) to pick it consistently across
/// desktop and mobile builds.
///
/// Panics if `url` does not parse.
pub fn dev_network_config(url: &str) -> NetworkConfig {
    NetworkConfig {
        bootstrap_url: url2::Url2::parse(url),
        relay_url: url2::Url2::parse(url),
        advanced: Some(serde_json::json!({
            "irohTransport": {
                "relayAllowPlainText": true
            },
            "k2Gossip": {
                "initiateBurstFactor": DEV_INITIATE_BURST_FACTOR
            }
        })),
        ..NetworkConfig::default()
    }
}

/// The local dev network URL, `http://<INTERNAL_IP>:<BOOTSTRAP_PORT>`, for
/// [`dev_network_config`].
///
/// - Desktop reads both variables at run time, defaulting to `127.0.0.1` and
///   `8888`. Set `INTERNAL_IP` to the machine's LAN address whenever a mobile
///   agent joins, so desktop advertises the same relay URL as the phone.
/// - Android and iOS read them at compile time, because the app runs on another
///   device that can't see the host's environment. Without `INTERNAL_IP`, Android
///   uses `10.0.2.2` (the emulator's alias for the host) and iOS `127.0.0.1` (the
///   simulator shares the host's network).
///
/// It is a macro so that the compile-time `option_env!` is expanded, and tracked
/// for rebuilds, in the app's crate rather than in this one.
#[macro_export]
macro_rules! dev_network_url {
    () => {{
        let (host, port): (::std::string::String, ::std::string::String) =
            if cfg!(target_os = "android") {
                (
                    option_env!("INTERNAL_IP").unwrap_or("10.0.2.2").into(),
                    option_env!("BOOTSTRAP_PORT").unwrap_or("8888").into(),
                )
            } else if cfg!(target_os = "ios") {
                (
                    option_env!("INTERNAL_IP").unwrap_or("127.0.0.1").into(),
                    option_env!("BOOTSTRAP_PORT").unwrap_or("8888").into(),
                )
            } else {
                (
                    ::std::env::var("INTERNAL_IP").unwrap_or_else(|_| "127.0.0.1".into()),
                    ::std::env::var("BOOTSTRAP_PORT").unwrap_or_else(|_| "8888".into()),
                )
            };
        ::std::format!("http://{}:{}", host, port)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_settings_reach_the_kitsune2_config() {
        let config = dev_network_config("http://192.168.1.20:8888");
        assert_eq!(config.bootstrap_url.as_str(), "http://192.168.1.20:8888/");
        assert_eq!(config.relay_url.as_str(), "http://192.168.1.20:8888/");

        // What the conductor hands kitsune2: `advanced` merged with the fields.
        let k2 = config.to_k2_config().unwrap();
        assert_eq!(k2["irohTransport"]["relayAllowPlainText"], true);
        assert_eq!(
            k2["k2Gossip"]["initiateBurstFactor"],
            DEV_INITIATE_BURST_FACTOR
        );
        assert_eq!(
            k2["irohTransport"]["relayUrl"]
                .as_str()
                .unwrap()
                .trim_end_matches('/'),
            "http://192.168.1.20:8888"
        );
    }

    #[test]
    fn dev_network_url_defaults_on_desktop() {
        if std::env::var("INTERNAL_IP").is_err() && std::env::var("BOOTSTRAP_PORT").is_err() {
            assert_eq!(dev_network_url!(), "http://127.0.0.1:8888");
        }
    }
}
