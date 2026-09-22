//! The hApp the runtime and plugin test suites install: one DNA with a minimal
//! integrity/coordinator zome pair from `zomes/`, compiled by `build.rs` against
//! the hdk of the holochain release this workspace depends on. Bumping holochain
//! rebuilds it, so there is no prebuilt `.happ` to regenerate.

use holochain_types::prelude::*;

/// The single role in the test hApp.
pub const ROLE_NAME: &str = "test";

/// The coordinator zome. It has `create_test_entry(String)`,
/// `get_test_entries(())` (empty on a fresh cell), and a `post_commit` hook
/// that emits an app signal for every commit.
pub const ZOME_NAME: &str = "test";

const INTEGRITY_ZOME_NAME: &str = "test_integrity";

const INTEGRITY_WASM: &[u8] =
    include_bytes!(concat!(env!("TEST_ZOMES_WASM_DIR"), "/test_integrity.wasm"));
const COORDINATOR_WASM: &[u8] = include_bytes!(concat!(
    env!("TEST_ZOMES_WASM_DIR"),
    "/test_coordinator.wasm"
));

/// The test hApp, packed as the bytes of a `.happ` file.
pub fn test_happ_bytes() -> Vec<u8> {
    let zome = |name: &str, dependencies| ZomeManifest {
        name: name.into(),
        hash: None,
        path: format!("{name}.wasm"),
        dependencies,
    };
    let dna_manifest = DnaManifest::current(
        "test-dna".into(),
        None,
        None,
        vec![zome(INTEGRITY_ZOME_NAME, None)],
        vec![zome(
            ZOME_NAME,
            Some(vec![ZomeDependency {
                name: INTEGRITY_ZOME_NAME.into(),
            }]),
        )],
    );
    let dna = DnaBundle::new(
        dna_manifest.try_into().unwrap(),
        vec![
            (
                format!("{INTEGRITY_ZOME_NAME}.wasm"),
                INTEGRITY_WASM.to_vec().into(),
            ),
            (
                format!("{ZOME_NAME}.wasm"),
                COORDINATOR_WASM.to_vec().into(),
            ),
        ],
    )
    .unwrap();

    let role = AppRoleManifest {
        name: ROLE_NAME.into(),
        dna: AppRoleDnaManifest {
            path: Some("test.dna".into()),
            modifiers: DnaModifiersOpt::none(),
            installed_hash: None,
            clone_limit: 0,
        },
        provisioning: Some(CellProvisioning::Create { deferred: false }),
    };
    let manifest = AppManifestCurrentBuilder::default()
        .name("test-happ".into())
        .description(None)
        .roles(vec![role])
        .build()
        .unwrap()
        .into();

    AppBundle::new(manifest, [("test.dna".to_string(), dna)])
        .unwrap()
        .pack()
        .unwrap()
        .to_vec()
}
