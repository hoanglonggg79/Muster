use std::path::{Path, PathBuf};

use muster_core::models::Account;
use muster_core::vault::{self, Branch, Format};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("khong doc duoc {}: {e}", path.display()))
}

fn fixture_password() -> String {
    let raw = fixture("manifest.json");
    let manifest: serde_json::Value =
        serde_json::from_slice(&raw).expect("manifest.json phai la JSON hop le");
    manifest["fixtures"]
        .as_array()
        .expect("manifest.fixtures phai la mang")
        .iter()
        .find(|entry| entry["kind"] == "password-locked")
        .and_then(|entry| entry["password"].as_str())
        .expect("manifest phai co mat khau cua fixture password-locked")
        .to_owned()
}

fn plain_accounts() -> Vec<Account> {
    let (accounts, format) =
        vault::read_accounts(&fixture("accountdata.plain.json"), None).expect("doc fixture plain");
    assert_eq!(format, Format::PlainJson);
    accounts
}

#[test]
fn plain_fixture_detects_as_plain_json_and_keeps_every_field() {
    let data = fixture("accountdata.plain.json");
    assert_eq!(vault::detect_branch(&data), Branch::PlainJson);

    let accounts = plain_accounts();
    assert_eq!(accounts.len(), 2, "fixture co dung 2 tai khoan");

    let alpha = &accounts[0];
    assert_eq!(alpha.username.as_deref(), Some("fixture_alpha"));
    assert_eq!(alpha.alias, "Alpha");
    assert_eq!(alpha.user_id, 100_000_001);
    assert_eq!(alpha.group.as_deref(), Some("Default"));
    assert_eq!(alpha.browser_tracker_id.as_deref(), Some("100000000000001"));
    assert!(alpha.valid);

    assert_eq!(
        alpha.fields.get("Window_Position_X").map(String::as_str),
        Some("120")
    );
    assert_eq!(
        alpha.fields.get("Window_Height").map(String::as_str),
        Some("740")
    );

    let beta = &accounts[1];
    assert_eq!(beta.username.as_deref(), Some("fixture_beta"));
    assert_eq!(beta.group.as_deref(), Some("007 Bank"));
}

#[test]
fn password_locked_fixture_detects_as_password_locked() {
    let data = fixture("accountdata.password-locked.bin");
    assert_eq!(vault::detect_branch(&data), Branch::PasswordLocked);
    assert!(data.starts_with(vault::RAM_HEADER));
}

#[test]
fn password_locked_fixture_opens_with_the_manifest_password_and_matches_the_plain_payload() {
    let data = fixture("accountdata.password-locked.bin");
    let password = fixture_password();

    let (accounts, format) =
        vault::read_accounts(&data, Some(&password)).expect("phai giai ma duoc fixture nhanh 1");

    assert_eq!(format, Format::PasswordLocked);
    assert_eq!(
        accounts,
        plain_accounts(),
        "nhanh 1 va nhanh 4 phai cho ra cung du lieu"
    );
}

#[test]
fn password_locked_fixture_rejects_a_wrong_password() {
    let data = fixture("accountdata.password-locked.bin");
    let err = vault::read_accounts(&data, Some("mat-khau-sai"))
        .expect_err("sai mat khau thi phai that bai");
    assert!(matches!(err, vault::VaultError::Decrypt), "gap: {err:?}");
}

#[test]
fn password_locked_fixture_requires_a_password_at_all() {
    let data = fixture("accountdata.password-locked.bin");
    let err = vault::open(&data, None).expect_err("thieu mat khau thi phai bao loi");
    assert!(
        matches!(err, vault::VaultError::PasswordRequired),
        "gap: {err:?}"
    );
}

#[test]
fn password_locked_fixture_survives_a_re_encrypt_round_trip() {
    let password = fixture_password();
    let original = plain_accounts();

    let plaintext = vault::accounts_to_bytes(&original).expect("tuan tu hoa");
    let sealed = vault::seal_password_locked(&plaintext, &password).expect("ma hoa lai");

    let temp = std::env::temp_dir().join("muster-vault-roundtrip.bin");
    std::fs::write(&temp, &sealed).expect("ghi file tam");

    let (back, format) =
        vault::read_accounts_from_path(&temp, Some(&password)).expect("doc lai file tam");
    let _ = std::fs::remove_file(&temp);

    assert_eq!(format, Format::PasswordLocked);
    assert_eq!(
        back, original,
        "du lieu phai nguyen ven sau mot vong ghi roi doc"
    );
}

fn open_dpapi_fixture(name: &str) -> Option<(Vec<Account>, Format)> {
    let data = fixture(name);
    match vault::read_accounts(&data, None) {
        Ok(result) => Some(result),
        Err(err) => {
            eprintln!(
                "bo qua {name}: may hien tai khong giai ma duoc blob DPAPI nay ({err}). \
                 Fixture duoc sinh tren mot may cu the, va DPAPI gan chat voi may do."
            );
            None
        }
    }
}

#[test]
fn dpapi_currentuser_fixture_matches_the_plain_payload_when_the_machine_can_open_it() {
    let Some((accounts, format)) = open_dpapi_fixture("accountdata.dpapi-currentuser.bin") else {
        return;
    };
    assert_eq!(format, Format::Dpapi);
    assert_eq!(accounts, plain_accounts());
}

#[test]
fn dpapi_localmachine_fixture_matches_the_plain_payload_when_the_machine_can_open_it() {
    let Some((accounts, format)) = open_dpapi_fixture("accountdata.dpapi-localmachine.bin") else {
        return;
    };
    assert_eq!(format, Format::Dpapi);
    assert_eq!(accounts, plain_accounts());
}

#[cfg(windows)]
#[test]
fn a_locally_sealed_blob_opens_through_the_public_entry_point() {
    let plaintext = br#"[{"Username":"alice"}]"#;
    let sealed =
        vault::seal_dpapi(plaintext, vault::dpapi::Scope::LocalMachine).expect("ma hoa DPAPI");

    let opened = vault::open(&sealed, None).expect("mo blob DPAPI");
    assert_eq!(opened.format, Format::Dpapi);
    assert_eq!(opened.plaintext.as_slice(), plaintext.as_slice());
}

#[test]
fn every_fixture_that_opens_yields_exactly_the_same_accounts() {
    let reference = plain_accounts();

    let password = fixture_password();
    let locked = vault::read_accounts(&fixture("accountdata.password-locked.bin"), Some(&password))
        .expect("nhanh 1 phai luon mo duoc");
    assert_eq!(locked.0, reference, "nhanh 1 khac nhanh 4");

    for name in [
        "accountdata.dpapi-currentuser.bin",
        "accountdata.dpapi-localmachine.bin",
    ] {
        if let Some((accounts, format)) = open_dpapi_fixture(name) {
            assert_eq!(format, Format::Dpapi, "nhan sai cho {name}");
            assert_eq!(accounts, reference, "{name} khac nhanh 4");
        }
    }
}

#[test]
fn the_manifest_lists_one_fixture_for_each_of_the_four_branches() {
    let raw = fixture("manifest.json");
    let manifest: serde_json::Value = serde_json::from_slice(&raw).expect("JSON hop le");
    let fixtures = manifest["fixtures"].as_array().expect("mang");

    let branches: Vec<i64> = fixtures
        .iter()
        .filter_map(|entry| entry["branch"].as_i64())
        .collect();
    for expected in 1..=4 {
        assert!(
            branches.contains(&expected),
            "manifest thieu fixture cho nhanh {expected}: {branches:?}"
        );
    }

    let verified = &manifest["verified_params"];
    assert_eq!(verified["kdf"], "Argon2i v1.3");
    assert_eq!(verified["t_cost"], vault::ARGON2_T_COST);
    assert_eq!(verified["m_cost_kib"], vault::ARGON2_M_COST_KIB);
    assert_eq!(verified["p_cost"], vault::ARGON2_P_COST);
    assert_eq!(verified["out_len"], vault::ARGON2_OUT_LEN);
    assert_eq!(verified["salt_len"], vault::SALT_LEN);
    assert_eq!(verified["nonce_len"], vault::NONCE_LEN);
    assert_eq!(verified["ram_header_len"], vault::RAM_HEADER.len());
}
