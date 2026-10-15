use crate::models::Account;

use super::account::{AccountStore, group_display_name, group_sort_key};

pub const DEFAULT_GROUP: &str = "Default";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GroupError {
    #[error("tên nhóm rỗng")]
    Empty,

    #[error("nhóm đích trùng với nhóm nguồn: {0}")]
    SameName(String),
}

#[must_use]
pub fn sorted_groups(accounts: &[Account]) -> Vec<String> {
    let mut groups: Vec<String> = accounts
        .iter()
        .map(|account| account.group_or_default().to_owned())
        .collect();

    groups.sort_by(|left, right| compare_groups(left, right));
    groups.dedup();
    groups
}

#[must_use]
pub fn compare_groups(left: &str, right: &str) -> std::cmp::Ordering {
    let left_key = group_sort_key(left);
    let right_key = group_sort_key(right);

    left_key
        .0
        .unwrap_or(u32::MAX)
        .cmp(&right_key.0.unwrap_or(u32::MAX))
        .then_with(|| left_key.1.cmp(right_key.1))
        .then_with(|| left.cmp(right))
}

#[must_use]
pub fn display_name(group: &str) -> &str {
    group_display_name(group)
}

#[must_use]
pub fn members(store: &AccountStore, group: &str) -> Vec<usize> {
    store
        .accounts()
        .iter()
        .enumerate()
        .filter(|(_, account)| account.group_or_default() == group)
        .map(|(index, _)| index)
        .collect()
}

pub fn validate_group_name(name: &str) -> Result<String, GroupError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(GroupError::Empty);
    }
    Ok(trimmed.to_owned())
}

pub fn assign_group(
    store: &mut AccountStore,
    index: usize,
    group: &str,
) -> Result<String, GroupError> {
    let name = validate_group_name(group)?;
    let normalized = if name == DEFAULT_GROUP {
        None
    } else {
        Some(name.clone())
    };
    let _ = store.set_group(index, normalized);
    Ok(name)
}

pub fn rename_group(store: &mut AccountStore, from: &str, to: &str) -> Result<usize, GroupError> {
    let from = validate_group_name(from)?;
    let to = validate_group_name(to)?;

    if from == to {
        return Err(GroupError::SameName(from));
    }

    let target = if to == DEFAULT_GROUP { None } else { Some(to) };
    let mut changed = 0;

    for index in 0..store.len() {
        let matches = store
            .get(index)
            .is_ok_and(|account| account.group_or_default() == from);
        if matches {
            let _ = store.set_group(index, target.clone());
            changed += 1;
        }
    }

    Ok(changed)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn account(username: &str, user_id: i64, group: Option<&str>) -> Account {
        Account {
            valid: true,
            security_token: Some("COOKIE".to_owned()),
            username: Some(username.to_owned()),
            last_use: String::new(),
            alias: String::new(),
            description: String::new(),
            password: String::new(),
            group: group.map(str::to_owned),
            user_id,
            fields: BTreeMap::new(),
            last_attempted_refresh: String::new(),
            browser_tracker_id: None,
            extra: BTreeMap::new(),
        }
    }

    fn store_of(groups: &[Option<&str>]) -> AccountStore {
        AccountStore::from_accounts(
            groups
                .iter()
                .enumerate()
                .map(|(index, group)| account(&format!("u{index}"), index as i64 + 1, *group))
                .collect(),
        )
    }

    #[test]
    fn groups_are_listed_without_duplicates_and_numbered_first() {
        let store = store_of(&[
            Some("10 Apple"),
            None,
            Some("9 Banana"),
            Some("007 Bank"),
            Some("1Main"),
            Some("10 Apple"),
        ]);

        assert_eq!(
            sorted_groups(store.accounts()),
            ["1Main", "007 Bank", "9 Banana", "10 Apple", "Default"],
            "so phai so sanh nhu so, va Default xuong cuoi"
        );
    }

    #[test]
    fn the_numeric_prefix_is_hidden_for_display() {
        assert_eq!(display_name("1Main"), "Main");
        assert_eq!(display_name("007 Bank"), "Bank");
        assert_eq!(display_name("Default"), "Default");
    }

    #[test]
    fn members_returns_the_positions_of_one_group() {
        let store = store_of(&[Some("1Main"), Some("007 Bank"), Some("1Main")]);
        assert_eq!(members(&store, "1Main"), [0, 2]);
        assert_eq!(members(&store, "007 Bank"), [1]);
        assert!(members(&store, "khong-ton-tai").is_empty());
    }

    #[test]
    fn an_empty_name_is_refused() {
        assert_eq!(validate_group_name("   "), Err(GroupError::Empty));
        assert_eq!(validate_group_name(" 1Main ").expect("hop le"), "1Main");
    }

    #[test]
    fn renaming_moves_every_member_and_reports_the_count() {
        let mut store = store_of(&[Some("1Main"), Some("007 Bank"), Some("1Main")]);

        let changed = rename_group(&mut store, "1Main", "2 Farm").expect("doi ten");

        assert_eq!(changed, 2);
        assert_eq!(store.get(0).expect("lay").group.as_deref(), Some("2 Farm"));
        assert_eq!(store.get(2).expect("lay").group.as_deref(), Some("2 Farm"));
        assert_eq!(
            store.get(1).expect("lay").group.as_deref(),
            Some("007 Bank")
        );
    }

    #[test]
    fn renaming_to_default_clears_the_group_field() {
        let mut store = store_of(&[Some("1Main")]);

        rename_group(&mut store, "1Main", "Default").expect("doi ten");

        assert_eq!(store.get(0).expect("lay").group, None);
        assert_eq!(store.get(0).expect("lay").group_or_default(), "Default");
    }

    #[test]
    fn renaming_onto_itself_is_refused() {
        let mut store = store_of(&[Some("1Main")]);
        assert_eq!(
            rename_group(&mut store, "1Main", "1Main"),
            Err(GroupError::SameName("1Main".to_owned()))
        );
    }

    #[test]
    fn assigning_default_clears_the_group() {
        let mut store = store_of(&[Some("1Main")]);
        assign_group(&mut store, 0, "Default").expect("gan nhom");
        assert_eq!(store.get(0).expect("lay").group, None);
    }

    #[test]
    fn assigning_a_named_group_sets_it() {
        let mut store = store_of(&[None]);
        assign_group(&mut store, 0, " 9 Banana ").expect("gan nhom");
        assert_eq!(
            store.get(0).expect("lay").group.as_deref(),
            Some("9 Banana")
        );
    }
}
