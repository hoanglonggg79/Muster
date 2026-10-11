use std::path::{Path, PathBuf};

use muster_core::models::Account;
use muster_core::vault::{self, Branch, Format};
use serde::Serialize;
use specta::Type;
use specta_typescript::Typescript;
use tauri_specta::{Builder, collect_commands};

#[derive(Debug, Serialize, Type)]
pub struct VaultInspection {
    pub path: String,
    pub size_bytes: u32,
    pub branch: Branch,
}

#[derive(Debug, Serialize, Type)]
pub struct VaultOpenSummary {
    pub path: String,
    pub format: Format,
    pub account_count: u32,
    pub usernames: Vec<String>,
}

#[tauri::command]
#[specta::specta]
fn vault_inspect(path: String) -> Result<VaultInspection, String> {
    let data = std::fs::read(&path).map_err(|error| format!("không đọc được {path}: {error}"))?;

    Ok(VaultInspection {
        path,
        size_bytes: u32::try_from(data.len()).unwrap_or(u32::MAX),
        branch: vault::detect_branch(&data),
    })
}

#[tauri::command]
#[specta::specta]
fn vault_open(path: String, password: Option<String>) -> Result<VaultOpenSummary, String> {
    let (accounts, format) = vault::read_accounts_from_path(Path::new(&path), password.as_deref())
        .map_err(|error| error.to_string())?;

    Ok(VaultOpenSummary {
        path,
        format,
        account_count: u32::try_from(accounts.len()).unwrap_or(u32::MAX),
        usernames: accounts
            .iter()
            .map(|account| account.display_name().to_owned())
            .collect(),
    })
}

#[must_use]
pub fn bindings_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/bindings.ts")
}

#[must_use]
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![vault_inspect, vault_open])
        .typ::<Account>()
}

#[must_use]
pub fn typescript_exporter() -> Typescript {
    Typescript::default()
}

pub fn export_bindings() -> Result<(), String> {
    specta_builder()
        .export(typescript_exporter(), bindings_path())
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();

    #[cfg(debug_assertions)]
    export_bindings().expect("không ghi được bindings TypeScript");

    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("không khởi động được ứng dụng");
}
