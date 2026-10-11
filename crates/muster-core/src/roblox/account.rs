//! # `roblox::account` — danh sách tài khoản và thao tác trên đó
//!
//! Tái lập ngữ nghĩa của `AccountManager.AccountsList` trong `RAM_Source/`, nhưng tách khỏi
//! WinForms: ở đây chỉ có dữ liệu và các phép biến đổi thuần, kiểm thử được bằng `cargo test`.
//!
//! ## Ba điểm hành vi lấy nguyên từ RAM
//!
//! 1. **Gộp theo `UserID`, không phải theo cookie.** `AccountManager.cs:577-594`: nếu đã có tài
//!    khoản cùng `UserID` thì RAM cập nhật cookie + mật khẩu + `LastUse` của bản cũ **tại chỗ**,
//!    thay vì thêm một dòng trùng. Nhờ vậy thêm lại cùng một tài khoản bằng cookie mới không sinh
//!    ra hai dòng.
//! 2. **Kéo thả vừa đổi vị trí vừa đổi nhóm.** `AccountManager.cs:1905-1925`: thả một tài khoản
//!    vào dòng nào thì nó nhận luôn nhóm của dòng đó.
//! 3. **Túi `Fields` là của người dùng.** RAM để vị trí cửa sổ (`Window_Position_X/Y`,
//!    `Window_Width/Height`) và mọi field người dùng tự thêm trong đó; mất túi này là mất dữ liệu
//!    (nguyên tắc bất biến #2). Xem [`crate::models::Account::get_field`] và hai hàm anh em.
//!
//! ## Vì sao là một kiểu bọc chứ không phải hàm trên `Vec<Account>`
//!
//! Vì có bất biến cần giữ: chỉ số luôn hợp lệ, và thao tác thêm phải đi qua luật gộp ở trên. Bọc
//! lại cho ta đúng một chỗ để kiểm soát điều đó, thay vì rải `Vec` khắp nơi rồi mỗi nơi tự nhớ.

use crate::models::Account;

/// Kết quả của một lần thêm tài khoản.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddOutcome {
    /// Chưa có tài khoản cùng `UserID` — đã thêm vào cuối danh sách.
    Added,
    /// Đã có tài khoản cùng `UserID` — đã cập nhật cookie/mật khẩu/`LastUse` của bản cũ.
    MergedIntoExisting,
}

/// Lỗi của các thao tác trên danh sách tài khoản.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AccountError {
    /// Chỉ số nằm ngoài danh sách.
    #[error("chỉ số {index} ngoài phạm vi (độ dài {len})")]
    IndexOutOfRange {
        /// Chỉ số đã yêu cầu.
        index: usize,
        /// Độ dài danh sách tại thời điểm gọi.
        len: usize,
    },

    /// Cố thêm một tài khoản chưa xác thực (`Valid = false`).
    ///
    /// RAM cũng từ chối trường hợp này (`AddAccount` trả `null` khi `account.Valid == false`) —
    /// một dòng không có `UserID` và `Username` thì vô dụng và chỉ làm rối danh sách.
    #[error("tài khoản chưa xác thực (Valid = false) nên không được thêm vào danh sách")]
    NotValid,
}

/// Danh sách tài khoản, giữ đúng ngữ nghĩa của `AccountManager.AccountsList`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AccountStore {
    accounts: Vec<Account>,
}

impl AccountStore {
    /// Danh sách rỗng.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Bọc một `Vec` có sẵn (ví dụ vừa đọc từ vault).
    #[must_use]
    pub fn from_accounts(accounts: Vec<Account>) -> Self {
        Self { accounts }
    }

    /// Toàn bộ tài khoản, theo đúng thứ tự đang lưu.
    #[must_use]
    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    /// Số tài khoản.
    #[must_use]
    pub fn len(&self) -> usize {
        self.accounts.len()
    }

    /// `true` nếu danh sách rỗng.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }

    /// Duyệt tài khoản theo thứ tự.
    pub fn iter(&self) -> std::slice::Iter<'_, Account> {
        self.accounts.iter()
    }

    /// Tài khoản tại `index`.
    ///
    /// # Errors
    ///
    /// Trả [`AccountError::IndexOutOfRange`] nếu `index` vượt quá độ dài.
    pub fn get(&self, index: usize) -> Result<&Account, AccountError> {
        self.accounts
            .get(index)
            .ok_or(AccountError::IndexOutOfRange {
                index,
                len: self.accounts.len(),
            })
    }

    /// Tài khoản tại `index`, cho sửa tại chỗ.
    ///
    /// # Errors
    ///
    /// Trả [`AccountError::IndexOutOfRange`] nếu `index` vượt quá độ dài.
    pub fn get_mut(&mut self, index: usize) -> Result<&mut Account, AccountError> {
        let len = self.accounts.len();
        self.accounts
            .get_mut(index)
            .ok_or(AccountError::IndexOutOfRange { index, len })
    }

    /// Vị trí của tài khoản có `UserID` cho trước.
    #[must_use]
    pub fn find_by_user_id(&self, user_id: i64) -> Option<usize> {
        self.accounts
            .iter()
            .position(|account| account.user_id == user_id)
    }

    /// Vị trí của tài khoản có `Username` cho trước, không phân biệt hoa thường.
    ///
    /// Không phân biệt hoa thường vì tên đăng nhập Roblox cũng vậy, và người dùng hay gõ lại tên
    /// với cách viết khác.
    #[must_use]
    pub fn find_by_username(&self, username: &str) -> Option<usize> {
        self.accounts.iter().position(|account| {
            account
                .username
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case(username))
        })
    }

    /// Thêm tài khoản, áp luật gộp theo `UserID` của RAM.
    ///
    /// Trả về kết quả và **vị trí** của tài khoản trong danh sách sau thao tác — chỗ gọi thường
    /// cần vị trí để cuộn tới dòng vừa thêm.
    ///
    /// # Errors
    ///
    /// Trả [`AccountError::NotValid`] nếu `account.valid == false`.
    pub fn add(&mut self, account: Account) -> Result<(AddOutcome, usize), AccountError> {
        if !account.valid {
            return Err(AccountError::NotValid);
        }

        // Chỉ gộp khi có UserID thật. `user_id == 0` nghĩa là chưa tra được UserID từ Roblox;
        // gộp theo nó sẽ nuốt nhầm mọi tài khoản chưa xác thực vào cùng một dòng.
        if account.user_id != 0 {
            if let Some(index) = self
                .accounts
                .iter()
                .position(|candidate| candidate.user_id == account.user_id)
            {
                // Đúng ba field mà RAM cập nhật (AccountManager.cs:583-585). Các field còn lại —
                // Alias, Description, Group, Fields — là của người dùng, không được đụng vào.
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

    /// Xoá tài khoản tại `index`, trả về tài khoản đã xoá.
    ///
    /// # Errors
    ///
    /// Trả [`AccountError::IndexOutOfRange`] nếu `index` vượt quá độ dài.
    pub fn remove(&mut self, index: usize) -> Result<Account, AccountError> {
        if index >= self.accounts.len() {
            return Err(AccountError::IndexOutOfRange {
                index,
                len: self.accounts.len(),
            });
        }
        Ok(self.accounts.remove(index))
    }

    /// Xoá tài khoản theo `UserID`, trả về tài khoản đã xoá nếu có.
    pub fn remove_by_user_id(&mut self, user_id: i64) -> Option<Account> {
        let index = self.find_by_user_id(user_id)?;
        Some(self.accounts.remove(index))
    }

    /// Gán nhóm cho tài khoản tại `index`. `None` nghĩa là trả về nhóm mặc định của RAM.
    ///
    /// # Errors
    ///
    /// Trả [`AccountError::IndexOutOfRange`] nếu `index` vượt quá độ dài.
    pub fn set_group(&mut self, index: usize, group: Option<String>) -> Result<(), AccountError> {
        self.get_mut(index)?.group = group;
        Ok(())
    }

    /// Di chuyển tài khoản, đồng thời cho nó nhận nhóm của dòng đích — đúng hành vi kéo thả của
    /// RAM (`AccountManager.cs:1905-1925`).
    ///
    /// `to` hiểu theo danh sách **trước khi** di chuyển, giống `DropTargetIndex` của
    /// ObjectListView. Ví dụ `[A,B,C]` rồi `move_account(0, 2)` cho ra `[B,C,A]` và `A` mang nhóm
    /// của `C`.
    ///
    /// # Errors
    ///
    /// Trả [`AccountError::IndexOutOfRange`] nếu `from` hoặc `to` vượt quá độ dài.
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

        // Sau khi remove, danh sách ngắn đi 1 nên kẹp lại cho an toàn.
        let landing = to.min(self.accounts.len());
        self.accounts.insert(landing, moved);
        Ok(())
    }

    /// Sắp xếp theo nhóm, giữ nguyên thứ tự tương đối trong cùng một nhóm.
    ///
    /// Dùng [`Account::group_or_default`] nên tài khoản không có nhóm rơi vào `"Default"` — đúng
    /// giá trị mặc định của RAM. Sắp xếp **ổn định** để tài khoản cùng nhóm không nhảy loạn sau
    /// mỗi lần sắp.
    pub fn sort_by_group(&mut self) {
        self.accounts
            .sort_by(|left, right| left.group_or_default().cmp(right.group_or_default()));
    }

    /// Lấy lại `Vec` bên trong, dùng khi cần ghi xuống vault.
    #[must_use]
    pub fn into_accounts(self) -> Vec<Account> {
        self.accounts
    }
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
        assert_eq!(merged.last_use, "2026-10-11T10:00:00");
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
