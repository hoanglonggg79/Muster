use crate::models::Account;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddOutcome {
    Added,
    MergedIntoExisting,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AccountError {
    #[error("chỉ số {index} ngoài phạm vi (độ dài {len})")]
    IndexOutOfRange { index: usize, len: usize },

    #[error("tài khoản chưa xác thực (Valid = false) nên không được thêm vào danh sách")]
    NotValid,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AccountStore {
    accounts: Vec<Account>,
}

impl AccountStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from_accounts(accounts: Vec<Account>) -> Self {
        Self { accounts }
    }

    #[must_use]
    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.accounts.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Account> {
        self.accounts.iter()
    }

    pub fn get(&self, index: usize) -> Result<&Account, AccountError> {
        self.accounts
            .get(index)
            .ok_or(AccountError::IndexOutOfRange {
                index,
                len: self.accounts.len(),
            })
    }

    pub fn get_mut(&mut self, index: usize) -> Result<&mut Account, AccountError> {
        let len = self.accounts.len();
        self.accounts
            .get_mut(index)
            .ok_or(AccountError::IndexOutOfRange { index, len })
    }

    #[must_use]
    pub fn find_by_user_id(&self, user_id: i64) -> Option<usize> {
        self.accounts
            .iter()
            .position(|account| account.user_id == user_id)
    }

    #[must_use]
    pub fn find_by_username(&self, username: &str) -> Option<usize> {
        self.accounts.iter().position(|account| {
            account
                .username
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case(username))
        })
    }

    pub fn add(&mut self, mut account: Account) -> Result<(AddOutcome, usize), AccountError> {
        if !account.valid {
            return Err(AccountError::NotValid);
        }

        account.last_use = crate::datetime::now_local_json_net();

        if account.user_id != 0 {
            if let Some(index) = self
                .accounts
                .iter()
                .position(|candidate| candidate.user_id == account.user_id)
            {
                let existing = &mut self.accounts[index];
                existing.security_token = account.security_token;
                existing.password = account.password;
                existing.last_use = account.last_use;

                return Ok((AddOutcome::MergedIntoExisting, index));
            }
        }

        self.accounts.push(account);
        Ok((AddOutcome::Added, self.accounts.len() - 1))
    }

    pub fn import_account(
        &mut self,
        account: Account,
    ) -> Result<(AddOutcome, usize), AccountError> {
        if !account.valid {
            return Err(AccountError::NotValid);
        }

        if account.user_id != 0 {
            if let Some(index) = self
                .accounts
                .iter()
                .position(|candidate| candidate.user_id == account.user_id)
            {
                let existing = &mut self.accounts[index];
                existing.security_token = account.security_token;
                existing.password = account.password;

                return Ok((AddOutcome::MergedIntoExisting, index));
            }
        }

        self.accounts.push(account);
        Ok((AddOutcome::Added, self.accounts.len() - 1))
    }

    pub fn remove(&mut self, index: usize) -> Result<Account, AccountError> {
        if index >= self.accounts.len() {
            return Err(AccountError::IndexOutOfRange {
                index,
                len: self.accounts.len(),
            });
        }
        Ok(self.accounts.remove(index))
    }

    pub fn remove_by_user_id(&mut self, user_id: i64) -> Option<Account> {
        let index = self.find_by_user_id(user_id)?;
        Some(self.accounts.remove(index))
    }

    pub fn set_group(&mut self, index: usize, group: Option<String>) -> Result<(), AccountError> {
        self.get_mut(index)?.group = group;
        Ok(())
    }

    pub fn move_account(&mut self, from: usize, to: usize) -> Result<(), AccountError> {
        let len = self.accounts.len();
        if from >= len {
            return Err(AccountError::IndexOutOfRange { index: from, len });
        }
        if to >= len {
            return Err(AccountError::IndexOutOfRange { index: to, len });
        }
        if from == to {
            return Ok(());
        }

        let target_group = self.accounts[to].group.clone();
        let mut moved = self.accounts.remove(from);
        moved.group = target_group;

        let landing = to.min(self.accounts.len());
        self.accounts.insert(landing, moved);
        Ok(())
    }

    pub fn sort_by_group(&mut self) {
        self.accounts.sort_by(|left, right| {
            let left_key = group_sort_key(left.group_or_default());
            let right_key = group_sort_key(right.group_or_default());

            left_key
                .0
                .unwrap_or(u32::MAX)
                .cmp(&right_key.0.unwrap_or(u32::MAX))
                .then_with(|| left_key.1.cmp(right_key.1))
                .then_with(|| left.group_or_default().cmp(right.group_or_default()))
        });
    }

    #[must_use]
    pub fn into_accounts(self) -> Vec<Account> {
        self.accounts
    }
}

#[must_use]
pub fn group_sort_key(group: &str) -> (Option<u32>, &str) {
    let bytes = group.as_bytes();
    let mut index = 0;
    let mut digits = 0;

    while index < bytes.len() && digits < 3 && bytes[index].is_ascii_digit() {
        index += 1;
        digits += 1;
    }

    if digits == 0 {
        return (None, group);
    }

    let Some(number) = group[..index].parse::<u32>().ok() else {
        return (None, group);
    };

    if index < bytes.len() && bytes[index] == b' ' {
        index += 1;
    }

    (Some(number), &group[index..])
}

#[must_use]
pub fn group_display_name(group: &str) -> &str {
    group_sort_key(group).1
}

impl FromIterator<Account> for AccountStore {
    fn from_iter<I: IntoIterator<Item = Account>>(iter: I) -> Self {
        Self {
            accounts: iter.into_iter().collect(),
        }
    }
}

impl<'a> IntoIterator for &'a AccountStore {
    type Item = &'a Account;
    type IntoIter = std::slice::Iter<'a, Account>;

    fn into_iter(self) -> Self::IntoIter {
        self.accounts.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(username: &str, user_id: i64, group: Option<&str>) -> Account {
        let mut account = Account {
            valid: true,
            security_token: Some(format!("COOKIE-{username}")),
            username: Some(username.to_owned()),
            last_use: String::new(),
            alias: String::new(),
            description: String::new(),
            password: String::new(),
            group: group.map(str::to_owned),
            user_id,
            fields: std::collections::BTreeMap::new(),
            last_attempted_refresh: String::new(),
            browser_tracker_id: None,
            extra: std::collections::BTreeMap::new(),
        };
        account.set_field("Window_Width", "880");
        account
    }

    fn store_of(names: &[(&str, i64)]) -> AccountStore {
        names
            .iter()
            .map(|(name, id)| account(name, *id, None))
            .collect()
    }

    #[test]
    fn a_valid_account_is_appended_and_its_index_is_returned() {
        let mut store = store_of(&[("alice", 1), ("bob", 2)]);
        let (outcome, index) = store.add(account("carol", 3, None)).expect("thêm");

        assert_eq!(outcome, AddOutcome::Added);
        assert_eq!(index, 2);
        assert_eq!(store.len(), 3);
    }

    #[test]
    fn an_invalid_account_is_refused_like_ram_does() {
        let mut store = AccountStore::new();
        let mut broken = account("ghost", 0, None);
        broken.valid = false;

        let error = store.add(broken).expect_err("phải từ chối");
        assert_eq!(error, AccountError::NotValid);
        assert!(store.is_empty());
    }

    #[test]
    fn adding_the_same_user_id_merges_instead_of_duplicating() {
        let mut store = AccountStore::new();
        store
            .add(account("alice", 100, None))
            .expect("thêm lần đầu");

        let mut again = account("alice", 100, None);
        again.security_token = Some("COOKIE-MOI".to_owned());
        again.password = "mat-khau-moi".to_owned();
        again.last_use = "2026-10-11T10:00:00".to_owned();

        let (outcome, index) = store.add(again).expect("thêm lần hai");

        assert_eq!(outcome, AddOutcome::MergedIntoExisting);
        assert_eq!(index, 0);
        assert_eq!(store.len(), 1, "khong duoc sinh dong trung");
        let merged = store.get(0).expect("lấy");
        assert_eq!(merged.cookie(), Some("COOKIE-MOI"));
        assert_eq!(merged.password, "mat-khau-moi");
        assert_ne!(
            merged.last_use, "2026-10-11T10:00:00",
            "RAM dóng dấ DateTime.Now, khong lay last_use cua account gui vao"
        );
        assert!(merged.last_use.len() >= 19, "phai la moc thoi gian moi");
    }

    #[test]
    fn importing_keeps_the_last_use_stamped_by_ram() {
        let mut store = AccountStore::new();
        let mut imported = account("alice", 100, None);
        imported.last_use = "2026-09-28T21:14:03.512".to_owned();

        store.import_account(imported).expect("nhập");

        let stored = store.get(0).expect("lấy");
        assert_eq!(
            stored.last_use, "2026-09-28T21:14:03.512",
            "nhập du lieu RAM phai giu nguyen last_use goc"
        );
    }

    #[test]
    fn importing_the_same_user_id_updates_the_credentials_only() {
        let mut store = AccountStore::new();
        let mut original = account("alice", 100, Some("007 Bank"));
        original.last_use = "2026-09-28T21:14:03.512".to_owned();
        original.alias = "Alice (main)".to_owned();
        store.import_account(original).expect("nhập lần nhất");

        let mut again = account("alice", 100, None);
        again.security_token = Some("COOKIE-MOI".to_owned());
        store.import_account(again).expect("nhập lần hai");

        let merged = store.get(0).expect("lấy");
        assert_eq!(merged.cookie(), Some("COOKIE-MOI"));
        assert_eq!(merged.last_use, "2026-09-28T21:14:03.512");
        assert_eq!(merged.group.as_deref(), Some("007 Bank"));
        assert_eq!(merged.alias, "Alice (main)");
    }

    #[test]
    fn merging_never_clobbers_the_fields_the_user_owns() {
        let mut store = AccountStore::new();
        let mut original = account("alice", 100, Some("007 Bank"));
        original.alias = "Alice (main)".to_owned();
        original.description = "ghi chu cua nguoi dung".to_owned();
        original.set_field("Window_Width", "1024");
        store.add(original).expect("thêm lần đầu");

        let mut refreshed = account("alice", 100, None);
        refreshed.security_token = Some("COOKIE-MOI".to_owned());
        store.add(refreshed).expect("thêm lần hai");

        let merged = store.get(0).expect("lấy");
        assert_eq!(merged.alias, "Alice (main)");
        assert_eq!(merged.description, "ghi chu cua nguoi dung");
        assert_eq!(merged.group.as_deref(), Some("007 Bank"));
        assert_eq!(merged.get_field("Window_Width"), "1024");
    }

    #[test]
    fn accounts_without_a_real_user_id_are_never_merged_together() {
        let mut store = AccountStore::new();
        store.add(account("alice", 0, None)).expect("thêm 1");
        let (outcome, index) = store.add(account("bob", 0, None)).expect("thêm 2");

        assert_eq!(
            outcome,
            AddOutcome::Added,
            "user_id 0 khong du tin cay de gop"
        );
        assert_eq!(index, 1);
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn find_by_username_ignores_case() {
        let store = store_of(&[("Alice", 1), ("bob", 2)]);
        assert_eq!(store.find_by_username("alice"), Some(0));
        assert_eq!(store.find_by_username("ALICE"), Some(0));
        assert_eq!(store.find_by_username("carol"), None);
    }

    #[test]
    fn find_by_user_id_returns_the_position() {
        let store = store_of(&[("alice", 1), ("bob", 2)]);
        assert_eq!(store.find_by_user_id(2), Some(1));
        assert_eq!(store.find_by_user_id(999), None);
    }

    #[test]
    fn remove_by_user_id_takes_the_account_out() {
        let mut store = store_of(&[("alice", 1), ("bob", 2)]);
        let removed = store.remove_by_user_id(1).expect("phải xoá được");

        assert_eq!(removed.username.as_deref(), Some("alice"));
        assert_eq!(store.len(), 1);
        assert_eq!(store.find_by_user_id(1), None);
        assert!(store.remove_by_user_id(1).is_none());
    }

    #[test]
    fn out_of_range_indices_are_reported_not_panicked() {
        let mut store = store_of(&[("alice", 1)]);
        assert_eq!(
            store.get(5).expect_err("phải lỗi"),
            AccountError::IndexOutOfRange { index: 5, len: 1 }
        );
        assert!(store.get_mut(5).is_err());
        assert!(store.remove(5).is_err());
        assert!(store.move_account(0, 9).is_err());
        assert!(store.move_account(9, 0).is_err());
    }

    #[test]
    fn moving_an_account_takes_the_group_of_the_target_row() {
        let mut store = AccountStore::from_accounts(vec![
            account("a", 1, Some("Nhom1")),
            account("b", 2, Some("Nhom2")),
            account("c", 3, Some("Nhom3")),
        ]);

        store.move_account(0, 2).expect("di chuyển");

        let names: Vec<&str> = store
            .iter()
            .map(|account| account.username.as_deref().unwrap_or_default())
            .collect();
        assert_eq!(names, ["b", "c", "a"], "A xuong cuoi");
        assert_eq!(
            store.get(2).expect("lấy").group.as_deref(),
            Some("Nhom3"),
            "A nhan nhom cua C"
        );
    }

    #[test]
    fn moving_upward_lands_at_the_front() {
        let mut store = AccountStore::from_accounts(vec![
            account("a", 1, Some("Nhom1")),
            account("b", 2, Some("Nhom2")),
            account("c", 3, Some("Nhom3")),
        ]);

        store.move_account(2, 0).expect("di chuyển");

        let names: Vec<&str> = store
            .iter()
            .map(|account| account.username.as_deref().unwrap_or_default())
            .collect();
        assert_eq!(names, ["c", "a", "b"]);
        assert_eq!(store.get(0).expect("lấy").group.as_deref(), Some("Nhom1"));
    }

    #[test]
    fn moving_onto_itself_changes_nothing() {
        let mut store = store_of(&[("a", 1), ("b", 2)]);
        store.move_account(1, 1).expect("di chuyển");
        assert_eq!(store.len(), 2);
        assert_eq!(store.get(1).expect("lấy").username.as_deref(), Some("b"));
    }

    #[test]
    fn sorting_by_group_is_stable_and_puts_ungrouped_accounts_in_default() {
        let mut store = AccountStore::from_accounts(vec![
            account("a", 1, Some("Zeta")),
            account("b", 2, None),
            account("c", 3, Some("Alpha")),
            account("d", 4, None),
        ]);

        store.sort_by_group();

        let pairs: Vec<(&str, &str)> = store
            .iter()
            .map(|account| {
                (
                    account.username.as_deref().unwrap_or_default(),
                    account.group_or_default(),
                )
            })
            .collect();
        assert_eq!(
            pairs,
            [
                ("c", "Alpha"),
                ("b", "Default"),
                ("d", "Default"),
                ("a", "Zeta")
            ],
            "trong cung nhom phai giu nguyen thu tu"
        );
    }

    #[test]
    fn groups_with_a_number_prefix_sort_by_their_number() {
        let mut store = AccountStore::from_accounts(vec![
            account("a", 1, Some("10 Apple")),
            account("b", 2, Some("9 Banana")),
            account("c", 3, Some("007 Bank")),
            account("d", 4, Some("1Main")),
            account("e", 5, None),
        ]);

        store.sort_by_group();

        let groups: Vec<&str> = store.iter().map(|a| a.group_or_default()).collect();
        assert_eq!(
            groups,
            ["1Main", "007 Bank", "9 Banana", "10 Apple", "Default"],
            "so phai duoc so sanh nhu so, khong phai nhu chuoi"
        );
    }

    #[test]
    fn the_group_prefix_is_hidden_from_the_display_name_like_ram_does() {
        assert_eq!(group_display_name("1Main"), "Main");
        assert_eq!(group_display_name("007 Bank"), "Bank");
        assert_eq!(group_display_name("Default"), "Default");
        assert_eq!(group_display_name("001 Apple"), "Apple");
        assert_eq!(group_display_name("12abc"), "abc");
    }

    #[test]
    fn only_three_leading_digits_count_as_a_prefix() {
        assert_eq!(group_sort_key("1234 Bank"), (Some(123), "4 Bank"));
        assert_eq!(group_sort_key("9999999x"), (Some(999), "9999x"));
        assert_eq!(group_sort_key("Bank"), (None, "Bank"));
        assert_eq!(group_sort_key("007"), (Some(7), ""));
    }

    #[test]
    fn set_group_can_clear_back_to_the_default() {
        let mut store = store_of(&[("a", 1)]);
        store
            .set_group(0, Some("007 Bank".to_owned()))
            .expect("gán");
        assert_eq!(store.get(0).expect("lấy").group_or_default(), "007 Bank");

        store.set_group(0, None).expect("xoá nhóm");
        assert_eq!(store.get(0).expect("lấy").group_or_default(), "Default");
        assert_eq!(store.get(0).expect("lấy").group, None);
    }

    #[test]
    fn the_store_round_trips_through_an_iterator() {
        let store = store_of(&[("a", 1), ("b", 2)]);
        let rebuilt: AccountStore = store.iter().cloned().collect();
        assert_eq!(rebuilt, store);
        assert_eq!(store.into_accounts().len(), 2);
    }
}
