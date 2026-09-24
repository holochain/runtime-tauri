const COMMANDS: &[&str] = &[
    "sign_zome_call",
    "sign_payload",
    "app_request",
    "get_user_network_config",
    "default_user_network_config",
    "set_user_network_config",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
