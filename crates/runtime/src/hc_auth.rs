//! hc-auth: authenticated bootstrap/relay support.
//!
//! For networks that gate bootstrap/relay behind authentication, the conductor
//! must present a base64 "auth material" proving control of an authorized agent
//! key. This module performs that flow against an auth server:
//!
//! 1. get-or-create a persistent Ed25519 agent key in lair,
//! 2. `GET <server>/now` → a base64url challenge payload,
//! 3. sign it with the agent key via lair,
//! 4. `PUT <server>/authenticate` with `{pubKey, payload, signature}` → a status,
//! 5. if authorized, build the base64 auth material to inject into `NetworkConfig`.
//!
//! The signature never covers the server's bytes as-is. Lair's `sign_by_pub_key`
//! is a plain Ed25519 signature, the same primitive Holochain uses to sign
//! actions, so signing whatever the auth server returned would let whoever
//! controls that endpoint obtain a signature over a serialized action, and a
//! consumer may well install its hApp with this very key (unyt does). Instead the
//! signed message is [`CHALLENGE_SIGNING_PREFIX`] followed by the challenge: a
//! message starting with that ASCII prefix cannot be a msgpack-encoded action.
//! The challenge must also be exactly [`CHALLENGE_LEN`] bytes, the size
//! hc-auth-server issues; a server that sends anything else is reported as an
//! auth failure, like one that is unreachable, and the conductor still boots.
//! The request names the scheme as [`SIGNING_SCHEME`] so the server can verify
//! the prefixed message, tell old raw-signature clients apart, and eventually
//! refuse them; see `validate_signature` in hc-auth-server's `routes_client.rs`.
//!
//! Ported from the unytco `tauri-plugin-holochain` fork (`feat/hc-auth`), adapted
//! to this crate's [`RuntimeError`] and the holochain `AgentPubKey` re-export.

use crate::{RuntimeError, RuntimeResult};
use base64::prelude::*;
use holochain::prelude::AgentPubKey;
use holochain_keystore::MetaLairClient;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn default_true() -> bool {
    true
}

/// Length of a challenge from `GET /now`: 8 bytes of timestamp and 24 of nonce.
/// Anything else is refused before signing.
pub const CHALLENGE_LEN: usize = 32;

/// Domain-separation prefix put in front of the challenge before signing, so a
/// challenge signature is only ever usable as one.
pub const CHALLENGE_SIGNING_PREFIX: &[u8] = b"hc-auth-challenge:";

/// Name of the signing scheme, sent as `scheme` in `/authenticate` bodies and the
/// auth material so the server verifies the prefixed message rather than the
/// raw one it accepts from older clients.
pub const SIGNING_SCHEME: &str = "hc-auth-challenge-v1";

/// A challenge from `GET /now`, decoded and length-checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Challenge(pub [u8; CHALLENGE_LEN]);

impl Challenge {
    /// Decode the base64url payload `GET /now` returned, refusing any other size.
    pub fn decode(payload_b64url: &str) -> RuntimeResult<Self> {
        let bytes = BASE64_URL_SAFE_NO_PAD
            .decode(payload_b64url)
            .map_err(|e| RuntimeError::HcAuth(format!("Invalid payload base64url: {e}")))?;
        let bytes: [u8; CHALLENGE_LEN] = bytes.as_slice().try_into().map_err(|_| {
            RuntimeError::HcAuth(format!(
                "Challenge must be {CHALLENGE_LEN} bytes, got {}",
                bytes.len()
            ))
        })?;
        Ok(Self(bytes))
    }

    /// The bytes actually signed: [`CHALLENGE_SIGNING_PREFIX`] followed by the
    /// challenge. Verifiers reconstruct the same message.
    pub fn signing_message(&self) -> Vec<u8> {
        let mut message = Vec::with_capacity(CHALLENGE_SIGNING_PREFIX.len() + CHALLENGE_LEN);
        message.extend_from_slice(CHALLENGE_SIGNING_PREFIX);
        message.extend_from_slice(&self.0);
        message
    }
}

/// Configuration for the hc-auth flow. `auth_bootstrap`/`auth_relay` select which
/// of the network's auth-material slots the resulting material is written to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HcAuthConfig {
    pub auth_server_url: String,
    #[serde(default = "default_true")]
    pub auth_bootstrap: bool,
    #[serde(default = "default_true")]
    pub auth_relay: bool,
}

/// Result of `PUT /authenticate`, mapped from the HTTP status code.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HcAuthStatus {
    Authorized,
    Pending,
    NotRegistered,
    Blocked,
    Failed(String),
}

/// Outcome of [`perform_auth_flow`]: the status plus the agent key (in both
/// holochain and raw-Ed25519-b64url forms) and, when authorized, the auth
/// material to set on the `NetworkConfig`.
#[derive(Debug, Clone)]
pub struct AuthFlowResult {
    pub status: HcAuthStatus,
    pub auth_material: Option<String>,
    pub agent_key: AgentPubKey,
    pub raw_ed25519_b64url: String,
}

fn auth_key_path(holochain_dir: &Path) -> PathBuf {
    holochain_dir.join("hc-auth-agent-key")
}

/// The 32 raw Ed25519 bytes of an agent key, as URL-safe base64 (no padding) —
/// the form the auth server expects for `pubKey`.
pub fn agent_pub_key_to_raw_ed25519_b64url(key: &AgentPubKey) -> String {
    let raw_32: &[u8] = key.get_raw_32();
    BASE64_URL_SAFE_NO_PAD.encode(raw_32)
}

/// Reuse the persisted hc-auth agent key (written to `<dir>/hc-auth-agent-key`)
/// if present, else mint a fresh Ed25519 keypair in lair and persist its pubkey.
pub async fn get_or_create_auth_key(
    keystore: &MetaLairClient,
    holochain_dir: &Path,
) -> RuntimeResult<AgentPubKey> {
    let key_path = auth_key_path(holochain_dir);

    if key_path.exists() {
        if let Ok(stored) = std::fs::read_to_string(&key_path) {
            let trimmed = stored.trim();
            if !trimmed.is_empty() {
                match AgentPubKey::try_from(trimmed) {
                    Ok(key) => {
                        log::info!("Reusing persisted hc-auth agent key");
                        return Ok(key);
                    }
                    Err(e) => {
                        log::warn!("Failed to parse persisted hc-auth key, generating new: {e:?}");
                    }
                }
            }
        }
    }

    log::info!("Generating new hc-auth agent key via Lair");
    let agent_pub_key = keystore
        .new_sign_keypair_random()
        .await
        .map_err(RuntimeError::Lair)?;

    let key_b64 = format!("{}", agent_pub_key);
    if let Err(e) = std::fs::write(&key_path, &key_b64) {
        log::error!("Failed to persist hc-auth agent key: {e}");
    }

    Ok(agent_pub_key)
}

/// `GET <server>/now` → base64url challenge payload.
pub async fn fetch_challenge(auth_server_url: &str) -> RuntimeResult<String> {
    let url = format!("{}/now", auth_server_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| RuntimeError::HcAuth(format!("GET /now failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(RuntimeError::HcAuth(format!(
            "GET /now returned {}",
            resp.status()
        )));
    }

    resp.text()
        .await
        .map_err(|e| RuntimeError::HcAuth(format!("GET /now body read failed: {e}")))
}

/// Sign `challenge` with `agent_key` via lair; returns the signature as URL-safe
/// base64 (no padding).
///
/// What is signed is [`Challenge::signing_message`], never the server's bytes
/// themselves: see the module docs for why this must not be a raw signing oracle.
pub async fn sign_challenge(
    keystore: &MetaLairClient,
    agent_key: &AgentPubKey,
    challenge: &Challenge,
) -> RuntimeResult<String> {
    let mut pub_key_32 = [0u8; 32];
    pub_key_32.copy_from_slice(agent_key.get_raw_32());

    let message = challenge.signing_message();
    let signature = keystore
        .lair_client()
        .sign_by_pub_key(pub_key_32.into(), None, Arc::from(message.as_slice()))
        .await
        .map_err(RuntimeError::Lair)?;

    Ok(BASE64_URL_SAFE_NO_PAD.encode(&signature.0[..]))
}

/// Build the base64 auth material (`base64(JSON{pubKey,payload,signature})`) the
/// conductor injects into bootstrap/relay requests.
pub fn build_auth_material(
    pubkey_b64url: &str,
    payload_b64url: &str,
    signature_b64url: &str,
) -> String {
    let auth_body = serde_json::json!({
        "pubKey": pubkey_b64url,
        "payload": payload_b64url,
        "signature": signature_b64url,
        "scheme": SIGNING_SCHEME,
    });
    BASE64_STANDARD.encode(auth_body.to_string().as_bytes())
}

/// `PUT <server>/authenticate` with the signed challenge → [`HcAuthStatus`].
pub async fn try_authenticate(
    auth_server_url: &str,
    pubkey_b64url: &str,
    payload_b64url: &str,
    signature_b64url: &str,
) -> RuntimeResult<HcAuthStatus> {
    let url = format!("{}/authenticate", auth_server_url.trim_end_matches('/'));
    let auth_body = serde_json::json!({
        "pubKey": pubkey_b64url,
        "payload": payload_b64url,
        "signature": signature_b64url,
        "scheme": SIGNING_SCHEME,
    });

    let client = reqwest::Client::new();
    let resp = client
        .put(&url)
        .header("Content-Type", "application/octet-stream")
        .body(auth_body.to_string())
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| RuntimeError::HcAuth(format!("PUT /authenticate failed: {e}")))?;

    match resp.status().as_u16() {
        200 => Ok(HcAuthStatus::Authorized),
        202 => Ok(HcAuthStatus::Pending),
        401 => Ok(HcAuthStatus::NotRegistered),
        403 => Ok(HcAuthStatus::Blocked),
        other => Err(RuntimeError::HcAuth(format!(
            "PUT /authenticate unexpected status: {other}"
        ))),
    }
}

/// Run the full hc-auth flow. Network/auth-server failures are returned as
/// `Ok(AuthFlowResult { status: Failed(..), .. })` (not `Err`) so the caller can
/// still bring the conductor up (e.g. open mode / retry later) while surfacing
/// the status.
pub async fn perform_auth_flow(
    keystore: &MetaLairClient,
    config: &HcAuthConfig,
    holochain_dir: &Path,
) -> RuntimeResult<AuthFlowResult> {
    let agent_key = get_or_create_auth_key(keystore, holochain_dir).await?;
    let raw_ed25519_b64url = agent_pub_key_to_raw_ed25519_b64url(&agent_key);

    log::info!(
        "hc-auth: Using agent key {}, raw Ed25519: {}",
        agent_key,
        raw_ed25519_b64url
    );

    let payload_b64url = match fetch_challenge(&config.auth_server_url).await {
        Ok(p) => p,
        Err(e) => {
            log::warn!("hc-auth: Could not reach auth server: {e}");
            return Ok(AuthFlowResult {
                status: HcAuthStatus::Failed(format!("Auth server unreachable: {e}")),
                auth_material: None,
                agent_key,
                raw_ed25519_b64url,
            });
        }
    };

    let challenge = match Challenge::decode(&payload_b64url) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("hc-auth: Auth server sent an unusable challenge: {e}");
            return Ok(AuthFlowResult {
                status: HcAuthStatus::Failed(format!("Unusable challenge: {e}")),
                auth_material: None,
                agent_key,
                raw_ed25519_b64url,
            });
        }
    };

    let signature_b64url = sign_challenge(keystore, &agent_key, &challenge).await?;

    let status = match try_authenticate(
        &config.auth_server_url,
        &raw_ed25519_b64url,
        &payload_b64url,
        &signature_b64url,
    )
    .await
    {
        Ok(s) => s,
        Err(e) => {
            log::warn!("hc-auth: Authentication request failed: {e}");
            return Ok(AuthFlowResult {
                status: HcAuthStatus::Failed(format!("{e}")),
                auth_material: None,
                agent_key,
                raw_ed25519_b64url,
            });
        }
    };

    let auth_material = if status == HcAuthStatus::Authorized {
        let material = build_auth_material(&raw_ed25519_b64url, &payload_b64url, &signature_b64url);
        log::info!("hc-auth: Key authorized, auth material generated");
        Some(material)
    } else {
        log::info!("hc-auth: Key status = {:?}, no auth material", status);
        None
    };

    Ok(AuthFlowResult {
        status,
        auth_material,
        agent_key,
        raw_ed25519_b64url,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lair_keystore_api::types::SharedLockedArray;
    use std::sync::Mutex;
    use tempfile::TempDir;

    async fn spawn_keystore(dir: &TempDir) -> MetaLairClient {
        let passphrase: SharedLockedArray = Arc::new(Mutex::new(
            lair_keystore_api::dependencies::sodoken::LockedArray::from(vec![0; 4]),
        ));
        holochain_keystore::lair_keystore::spawn_lair_keystore_in_proc(
            &dir.path().join("lair-keystore-config.yaml"),
            passphrase,
        )
        .await
        .expect("in-proc lair must spawn")
    }

    fn challenge_b64url(bytes: &[u8]) -> String {
        BASE64_URL_SAFE_NO_PAD.encode(bytes)
    }

    #[test]
    fn signing_message_is_prefix_then_challenge() {
        let challenge = Challenge([7u8; CHALLENGE_LEN]);
        let message = challenge.signing_message();
        assert_eq!(
            &message[..CHALLENGE_SIGNING_PREFIX.len()],
            CHALLENGE_SIGNING_PREFIX
        );
        assert_eq!(&message[CHALLENGE_SIGNING_PREFIX.len()..], &challenge.0);
        // A msgpack map or array marker byte starts every serialized action;
        // the prefix must never look like one.
        assert!(CHALLENGE_SIGNING_PREFIX[0].is_ascii_alphabetic());
    }

    #[test]
    fn challenges_of_the_wrong_length_are_refused_before_signing() {
        // Long enough to be a serialized action, and a zome-call-hash-sized
        // payload with one byte missing: neither is a challenge.
        for wrong in [vec![0x80u8; 200], vec![1u8; CHALLENGE_LEN - 1], vec![]] {
            let result = Challenge::decode(&challenge_b64url(&wrong));
            assert!(
                matches!(result, Err(RuntimeError::HcAuth(ref m)) if m.contains("32 bytes")),
                "expected a length error, got {result:?}"
            );
        }
        assert!(matches!(
            Challenge::decode("not*base64url"),
            Err(RuntimeError::HcAuth(_))
        ));
        let right = [9u8; CHALLENGE_LEN];
        assert_eq!(
            Challenge::decode(&challenge_b64url(&right)).unwrap(),
            Challenge(right)
        );
    }

    #[test]
    fn requests_name_the_signing_scheme() {
        let material = build_auth_material("pk", "payload", "sig");
        let body: serde_json::Value =
            serde_json::from_slice(&BASE64_STANDARD.decode(material).unwrap()).unwrap();
        assert_eq!(body["scheme"], SIGNING_SCHEME);
        assert_eq!(body["pubKey"], "pk");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn challenge_signature_covers_the_prefixed_message_not_the_raw_bytes() {
        let dir = TempDir::new().unwrap();
        let keystore = spawn_keystore(&dir).await;
        let agent_key = get_or_create_auth_key(&keystore, dir.path()).await.unwrap();

        let challenge = Challenge([42u8; CHALLENGE_LEN]);
        let signature_b64 = sign_challenge(&keystore, &agent_key, &challenge)
            .await
            .expect("a 32-byte challenge must sign");

        let signature: [u8; 64] = BASE64_URL_SAFE_NO_PAD
            .decode(signature_b64)
            .unwrap()
            .try_into()
            .unwrap();
        let pub_key: [u8; 32] = agent_key.get_raw_32().try_into().unwrap();

        assert!(
            sodoken::sign::verify_detached(&signature, &challenge.signing_message(), &pub_key),
            "signature must verify against the domain-separated message"
        );
        assert!(
            !sodoken::sign::verify_detached(&signature, &challenge.0, &pub_key),
            "signature must not be a valid signature over the server's raw bytes"
        );
    }
}
