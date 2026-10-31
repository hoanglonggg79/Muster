import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { commands } from "../bindings";
import type {
  Account,
  BrowserProgress,
  BrowserStatus,
  FavoriteGame_Serialize,
  GameDetails,
  GroupInfo,
  RecentGame_Serialize,
  ServerInfo_Serialize,
} from "../bindings";
import {
  type LaunchPhase,
  type NavigationTab,
  savedJobId,
  savedPlaceId,
} from "../lib/accountUtils";


interface EditData {
  alias: string;
  group: string;
  description: string;
  placeId: string;
  jobId: string;
}

interface AppState {
  activeTab: NavigationTab;
  accounts: Account[];
  searchQuery: string;
  viewMode: "grid" | "table";
  placeId: string;
  jobId: string;
  selectedUserId: number | null;
  multiRoblox: boolean;
  unlockFps: boolean;
  maxFps: number;
  toast: string | null;
  selectedIds: number[];
  launchPhases: Record<number, LaunchPhase>;
  presence: Record<number, number>;
  batchRunning: boolean;
  showAddModal: boolean;
  showImportModal: boolean;
  editAccount: Account | null;

  groups: GroupInfo[];
  groupFilter: string | null;
  servers: ServerInfo_Serialize[];
  serversCursor: string | null;
  serversLoading: boolean;
  favorites: FavoriteGame_Serialize[];
  recent: RecentGame_Serialize[];
  games: Record<number, GameDetails>;
  gameThumbnails: Record<number, string>;
  gamesLoading: boolean;
  browser: BrowserStatus | null;
  browserProgress: BrowserProgress | null;
  browserBusy: boolean;

  setActiveTab: (tab: NavigationTab) => void;
  setSearchQuery: (query: string) => void;
  setViewMode: (mode: "grid" | "table") => void;
  setPlaceId: (id: string) => void;
  setJobId: (id: string) => void;
  setSelectedUserId: (id: number | null) => void;
  setShowAddModal: (show: boolean) => void;
  setShowImportModal: (show: boolean) => void;
  setEditAccount: (acc: Account | null) => void;
  notify: (msg: string) => void;

  toggleSelect: (userId: number) => void;
  toggleSelectAll: () => void;
  refreshPresence: (ids: number[]) => Promise<void>;
  refreshAccounts: () => Promise<void>;
  toggleMultiRoblox: () => Promise<void>;
  applyFps: (newUnlock: boolean, fpsVal: number) => Promise<void>;
  launchAccount: (userId: number, acc?: Account) => Promise<boolean>;
  launchSelected: () => Promise<void>;
  saveEdit: (data: EditData) => Promise<boolean>;
  refreshAccount: (userId: number) => Promise<void>;
  removeAccount: (userId: number, name: string) => Promise<void>;
  addAccountCookie: (cookie: string, alias: string | null) => Promise<boolean>;
  addAccountCredentials: (
    user: string,
    pass: string,
    alias: string | null
  ) => Promise<boolean>;
  initAppState: () => Promise<void>;
  importRamVault: (dir: string, pass: string | null) => Promise<boolean>;

  refreshGroups: () => Promise<void>;
  setGroupFilter: (group: string | null) => void;
  renameGroup: (from: string, to: string) => Promise<void>;
  assignGroup: (userId: number, group: string) => Promise<void>;
  refreshServers: (placeId: string, cursor?: string | null) => Promise<void>;
  pickServer: (placeId: string, lowestPlayers: boolean) => Promise<void>;
  joinServer: (userId: number, placeId: number, jobId: string) => Promise<boolean>;
  refreshFavorites: () => Promise<void>;
  toggleFavorite: (name: string, placeId: number) => Promise<void>;
  refreshRecent: () => Promise<void>;
  pushRecent: (name: string, placeId: number) => Promise<void>;
  refreshGames: (placeIds: number[]) => Promise<void>;
  refreshBrowser: () => Promise<void>;
  downloadChromium: () => Promise<boolean>;
  openBrowserLogin: (
    username: string | null,
    password: string | null
  ) => Promise<boolean>;
}

export const useAppStore = create<AppState>((set, get) => ({
  activeTab: "accounts",
  accounts: [],
  searchQuery: "",
  viewMode: "grid",
  placeId: "4483381587",
  jobId: "",
  selectedUserId: null,
  multiRoblox: false,
  unlockFps: false,
  maxFps: 60,
  toast: null,
  selectedIds: [],
  launchPhases: {},
  presence: {},
  batchRunning: false,
  showAddModal: false,
  showImportModal: false,
  editAccount: null,

  groups: [],
  groupFilter: null,
  servers: [],
  serversCursor: null,
  serversLoading: false,
  favorites: [],
  recent: [],
  games: {},
  gameThumbnails: {},
  gamesLoading: false,
  browser: null,
  browserProgress: null,
  browserBusy: false,

  setActiveTab: (tab) => set({ activeTab: tab }),
  setSearchQuery: (searchQuery) => set({ searchQuery }),
  setViewMode: (viewMode) => set({ viewMode }),
  setPlaceId: (placeId) => set({ placeId }),
  setJobId: (jobId) => set({ jobId }),
  setSelectedUserId: (selectedUserId) => set({ selectedUserId }),
  setShowAddModal: (showAddModal) => set({ showAddModal }),
  setShowImportModal: (showImportModal) => set({ showImportModal }),
  setEditAccount: (editAccount) => set({ editAccount }),

  notify: (msg) => {
    set({ toast: msg });
    setTimeout(() => {
      if (get().toast === msg) {
        set({ toast: null });
      }
    }, 3500);
  },

  toggleSelect: (userId) => {
    const current = get().selectedIds;
    set({
      selectedIds: current.includes(userId)
        ? current.filter((id) => id !== userId)
        : [...current, userId],
    });
  },

  toggleSelectAll: () => {
    const { accounts, searchQuery, selectedIds } = get();
    const q = searchQuery.toLowerCase().trim();
    const visible = accounts
      .filter((acc) => {
        if (!q) return true;
        const name = (
          acc.Fields?.DisplayName ||
          acc.Alias ||
          acc.Username ||
          ""
        ).toLowerCase();
        const username = (acc.Username || "").toLowerCase();
        const uid = String(acc.UserID || "");
        return name.includes(q) || username.includes(q) || uid.includes(q);
      })
      .map((acc) => acc.UserID ?? 0)
      .filter((id) => id > 0);

    const allSelected =
      visible.length > 0 && visible.every((id) => selectedIds.includes(id));
    set({ selectedIds: allSelected ? [] : visible });
  },

  refreshPresence: async (ids) => {
    if (ids.length === 0) {
      set({ presence: {} });
      return;
    }
    try {
      const res = await commands.presenceBatch(ids);
      if (res.status === "ok") {
        const map: Record<number, number> = {};
        for (const item of res.data) {
          if (item.user_id !== null && item.user_id !== undefined) {
            map[item.user_id] = item.presence_type;
          }
        }
        set({ presence: map });
      }
    } catch {
      set({ presence: {} });
    }
  },

  refreshAccounts: async () => {
    try {
      const res = await commands.accountList();
      if (res.status === "ok") {
        const accs = res.data;
        set({ accounts: accs });
        if (accs.length > 0 && get().selectedUserId === null) {
          const firstUid = accs[0]?.UserID;
          if (firstUid) {
            set({ selectedUserId: firstUid });
          }
        }
        const validIds = accs
          .map((acc) => acc.UserID ?? 0)
          .filter((id) => id > 0);
        await get().refreshPresence(validIds);
      }
    } catch (e) {
      get().notify(`Loi tai danh sach: ${String(e)}`);
    }
  },

  initAppState: async () => {
    try {
      const res = await commands.appInitState();
      if (res.status === "ok") {
        set({
          unlockFps: res.data.unlock_fps,
          maxFps: res.data.max_fps > 0 ? res.data.max_fps : 60,
          multiRoblox: res.data.multi_roblox,
        });
      }
    } catch {
      /* ignore – settings file may not exist yet */
    }
  },

  toggleMultiRoblox: async () => {
    const nextVal = !get().multiRoblox;
    try {
      const res = await commands.multiRobloxToggle(nextVal);
      if (res.status === "ok") {
        set({ multiRoblox: res.data });
        get().notify(
          res.data ? "Da kich hoat Multi-Roblox" : "Da tat Multi-Roblox"
        );
      } else {
        set({ multiRoblox: false });
        get().notify(`Khong bat duoc Multi-Roblox: ${res.error}`);
      }
    } catch (e) {
      set({ multiRoblox: false });
      get().notify(`Loi Multi-Roblox: ${String(e)}`);
    }
  },

  applyFps: async (newUnlock, fpsVal) => {
    try {
      const res = await commands.patchApplyFps(newUnlock, fpsVal);
      if (res.status === "ok") {
        set({ unlockFps: newUnlock, maxFps: fpsVal });
        get().notify(res.data);
      } else {
        get().notify(`Loi FPS: ${res.error}`);
      }
    } catch (e) {
      get().notify(`Loi FPS: ${String(e)}`);
    }
  },

  launchAccount: async (userId, acc) => {
    const { placeId, jobId, notify } = get();
    const rawPlace = acc ? savedPlaceId(acc, placeId) : placeId;
    const pid = Number(String(rawPlace).replace(/[^0-9]/g, ""));
    if (!pid || Number.isNaN(pid)) {
      notify("Vui long nhap Place ID hop le");
      return false;
    }
    const job = acc ? savedJobId(acc, jobId) : jobId.trim() || null;
    notify("Dang khoi chay Roblox client...");
    try {
      const res = await commands.accountLaunch(userId, pid, job);
      if (res.status === "ok") {
        notify(res.data);
        return true;
      }
      notify(`Loi khoi chay: ${res.error}`);
      return false;
    } catch (e) {
      notify(`Loi launch: ${String(e)}`);
      return false;
    }
  },

  launchSelected: async () => {
    const { accounts, selectedIds, launchAccount, notify } = get();
    const chosen = accounts.filter((acc) =>
      selectedIds.includes(acc.UserID ?? 0)
    );
    if (chosen.length === 0) {
      notify("Chua chon tai khoan nao de khoi chay");
      return;
    }

    set({ batchRunning: true });
    let okCount = 0;
    const failed: string[] = [];

    for (let index = 0; index < chosen.length; index++) {
      const acc = chosen[index];
      if (!acc) continue;
      const uid = acc.UserID ?? 0;
      const name = acc.Fields?.DisplayName || acc.Username || String(uid);

      set((state) => ({
        launchPhases: { ...state.launchPhases, [uid]: "launching" },
      }));
      const ok = await launchAccount(uid, acc);
      set((state) => ({
        launchPhases: {
          ...state.launchPhases,
          [uid]: ok ? "done" : "error",
        },
      }));

      if (ok) {
        okCount++;
      } else {
        failed.push(name);
      }

      if (index < chosen.length - 1) {
        await new Promise((resolve) => setTimeout(resolve, 2000));
      }
    }

    set({ batchRunning: false, selectedIds: [] });
    notify(
      failed.length === 0
        ? `Da khoi chay ${okCount} tai khoan`
        : `Hoan tat: ${okCount} thanh cong, loi ${failed.length} (${failed.join(", ")})`
    );
    setTimeout(() => set({ launchPhases: {} }), 6000);
  },

  saveEdit: async (data) => {
    const { editAccount, notify, refreshAccounts } = get();
    if (!editAccount) return false;
    try {
      const res = await commands.accountUpdate(
        editAccount.UserID ?? 0,
        data.alias,
        data.group.trim() || null,
        data.description,
        data.placeId.trim() || null,
        data.jobId.trim() || null
      );
      if (res.status === "ok") {
        notify(`Da cap nhat ${res.data.Username ?? "tai khoan"}`);
        set({ editAccount: null });
        await refreshAccounts();
        return true;
      }
      notify(`Loi cap nhat: ${res.error}`);
      return false;
    } catch (e) {
      notify(`Loi: ${String(e)}`);
      return false;
    }
  },

  refreshAccount: async (userId) => {
    const { notify, refreshAccounts } = get();
    try {
      const res = await commands.accountRefresh(userId);
      if (res.status === "ok") {
        notify(`Da cap nhat: ${res.data.Username ?? "Tai khoan"}`);
        await refreshAccounts();
      } else {
        notify(`Loi lam moi: ${res.error}`);
      }
    } catch (e) {
      notify(`Loi: ${String(e)}`);
    }
  },

  removeAccount: async (userId, name) => {
    const { notify, refreshAccounts } = get();
    if (!confirm(`Xac nhan xoa tai khoan ${name}?`)) return;
    try {
      const res = await commands.accountRemove(userId);
      if (res.status === "ok") {
        notify(`Da xoa tai khoan ${name}`);
        await refreshAccounts();
      } else {
        notify(`Loi: ${res.error}`);
      }
    } catch (e) {
      notify(`Loi: ${String(e)}`);
    }
  },

  addAccountCookie: async (cookie, alias) => {
    const { notify, refreshAccounts } = get();
    try {
      const res = await commands.accountAddCookie(cookie, alias);
      if (res.status === "ok") {
        notify(`Them thanh cong ${res.data.Username ?? "tai khoan"}`);
        set({ showAddModal: false });
        await refreshAccounts();
        return true;
      }
      notify(`Loi them: ${res.error}`);
      return false;
    } catch (e) {
      notify(`Loi: ${String(e)}`);
      return false;
    }
  },

  addAccountCredentials: async (user, pass, alias) => {
    const { notify, refreshAccounts } = get();
    try {
      const res = await commands.accountAddCredentials(user, pass, alias);
      if (res.status === "ok") {
        notify(`Dang nhap thanh cong: ${res.data.Username ?? ""}`);
        set({ showAddModal: false });
        await refreshAccounts();
        return true;
      }
      notify(`Dang nhap that bai: ${res.error}`);
      return false;
    } catch (e) {
      notify(`Loi: ${String(e)}`);
      return false;
    }
  },

  importRamVault: async (dir, pass) => {
    const { notify, refreshAccounts } = get();
    try {
      const res = await commands.vaultImportRam(dir, pass);
      if (res.status === "ok") {
        notify(`Da nhap thanh cong ${res.data} tai khoan tu RAM!`);
        set({ showImportModal: false });
        await refreshAccounts();
        await get().refreshGroups();
        return true;
      }
      notify(`Loi nhap: ${res.error}`);
      return false;
    } catch (e) {
      notify(`Loi import: ${String(e)}`);
      return false;
    }
  },

  refreshGroups: async () => {
    try {
      const res = await commands.groupsList();
      if (res.status === "ok") {
        set({ groups: res.data });
      }
    } catch {
      set({ groups: [] });
    }
  },

  setGroupFilter: (group) => set({ groupFilter: group }),

  renameGroup: async (from, to) => {
    const { notify } = get();
    try {
      const res = await commands.groupRename(from, to);
      if (res.status === "ok") {
        notify(`Da doi ten nhom cho ${res.data} tai khoan`);
        await get().refreshAccounts();
        await get().refreshGroups();
        set({ groupFilter: null });
      } else {
        notify(`Loi doi ten nhom: ${res.error}`);
      }
    } catch (e) {
      notify(`Loi: ${String(e)}`);
    }
  },

  assignGroup: async (userId, group) => {
    const { notify } = get();
    try {
      const res = await commands.groupAssign(userId, group);
      if (res.status === "ok") {
        await get().refreshAccounts();
        await get().refreshGroups();
      } else {
        notify(`Loi gan nhom: ${res.error}`);
      }
    } catch (e) {
      notify(`Loi: ${String(e)}`);
    }
  },

  refreshServers: async (placeId, cursor) => {
    const pid = Number(String(placeId).replace(/[^0-9]/g, ""));
    if (!pid || Number.isNaN(pid)) {
      get().notify("Vui long nhap Place ID hop le");
      return;
    }

    set({ serversLoading: true });
    try {
      const res = await commands.serversList(pid, cursor ?? null);
      if (res.status === "ok") {
        const page = res.data;
        const list = cursor ? [...get().servers, ...page.data] : page.data;
        set({ servers: list, serversCursor: page.nextPageCursor ?? null });
      } else {
        get().notify(`Loi tai server: ${res.error}`);
        set({ servers: [], serversCursor: null });
      }
    } catch (e) {
      get().notify(`Loi: ${String(e)}`);
      set({ servers: [], serversCursor: null });
    } finally {
      set({ serversLoading: false });
    }
  },

  pickServer: async (placeId, lowestPlayers) => {
    const pid = Number(String(placeId).replace(/[^0-9]/g, ""));
    if (!pid || Number.isNaN(pid)) {
      get().notify("Vui long nhap Place ID hop le");
      return;
    }

    set({ serversLoading: true });
    try {
      const res = await commands.serversPickJob(pid, 5, lowestPlayers);
      if (res.status === "ok") {
        if (res.data) {
          set({ jobId: res.data });
          get().notify(`Da chon server ${res.data.slice(0, 12)}...`);
        } else {
          get().notify("Khong tim thay server nao con cho trong");
        }
      } else {
        get().notify(`Loi chon server: ${res.error}`);
      }
    } catch (e) {
      get().notify(`Loi: ${String(e)}`);
    } finally {
      set({ serversLoading: false });
    }
  },

  joinServer: async (userId, placeId, jobId) => {
    set({ placeId: String(placeId), jobId });
    return get().launchAccount(userId);
  },

  refreshFavorites: async () => {
    try {
      const res = await commands.favoritesList();
      if (res.status === "ok") {
        set({ favorites: res.data });
      }
    } catch {
      set({ favorites: [] });
    }
  },

  toggleFavorite: async (name, placeId) => {
    const { favorites, notify } = get();
    const existing = favorites.find((item) => item.placeId === placeId);
    try {
      const res = existing
        ? await commands.favoriteRemove(placeId, existing.privateServer ?? null)
        : await commands.favoriteAdd(name, placeId, null);

      if (res.status === "ok") {
        set({ favorites: res.data });
        notify(existing ? "Da bo yeu thich" : "Da them vao yeu thich");
      } else {
        notify(`Loi yeu thich: ${res.error}`);
      }
    } catch (e) {
      notify(`Loi: ${String(e)}`);
    }
  },

  refreshRecent: async () => {
    try {
      const res = await commands.recentGames();
      if (res.status === "ok") {
        set({ recent: res.data });
      }
    } catch {
      set({ recent: [] });
    }
  },

  pushRecent: async (name, placeId) => {
    try {
      const res = await commands.recentPush(name, placeId);
      if (res.status === "ok") {
        set({ recent: res.data });
      }
    } catch {
      /* danh sach gan day khong quan trong bang viec khoi chay */
    }
  },

  refreshGames: async (placeIds) => {
    const valid = placeIds.filter((id) => id != null && !Number.isNaN(id));
    if (valid.length === 0) return;
    set({ gamesLoading: true });
    try {
      const detailsRes = await commands.gameDetails(valid, null);
      if (detailsRes.status === "ok") {
        const next: Record<number, GameDetails> = { ...get().games };
        for (const game of detailsRes.data) {
          if (game.placeId != null) next[game.placeId] = game;
        }
        set({ games: next });
      }
      const thumbRes = await commands.thumbnailsBatch(
        valid.map((id) => ({
          asset_id: id,
          kind: "GameIcon",
          size: "150x150",
          format: null,
        }))
      );
      if (thumbRes.status === "ok") {
        const nextThumb: Record<number, string> = { ...get().gameThumbnails };
        for (const thumb of thumbRes.data) {
          if (
            thumb.asset_id != null &&
            thumb.url &&
            thumb.url !== "UNK" &&
            /^https?:\/\//i.test(thumb.url)
          ) {
            nextThumb[thumb.asset_id] = thumb.url;
          }
        }
        set({ gameThumbnails: nextThumb });
      }
    } catch {
      /* dữ liệu trò chơi chỉ là enhancement, không chặn khiển.launch */
    } finally {
      set({ gamesLoading: false });
    }
  },


  refreshBrowser: async () => {
    try {
      const status = await commands.browserStatus();
      set({ browser: status });
    } catch {
      set({ browser: null });
    }
  },

  downloadChromium: async () => {
    const { notify } = get();
    set({
      browserBusy: true,
      browserProgress: {
        phase: "connecting",
        downloaded: 0,
        total: 0,
        percent: 0,
      },
    });

    let unlisten: (() => void) | undefined;
    try {
      unlisten = await listen<BrowserProgress>("browser-download", (event) => {
        set({ browserProgress: event.payload });
      });
      const res = await commands.browserDownloadChromium();
      if (res.status === "ok") {
        notify("Chromium da san sang");
        await get().refreshBrowser();
        return true;
      }
      notify(`Loi tai Chromium: ${res.error}`);
      return false;
    } catch (e) {
      notify(`Loi tai Chromium: ${String(e)}`);
      return false;
    } finally {
      unlisten?.();
      set({ browserBusy: false, browserProgress: null });
    }
  },

  openBrowserLogin: async (username, password) => {
    const { notify, refreshAccounts } = get();
    set({ browserBusy: true });
    notify(
      username
        ? "Dang mo trinh duyet va tu dien thong tin dang nhap..."
        : "Dang mo trinh duyet - hay dang nhap Roblox trong cua so vua mo"
    );

    try {
      const res = await commands.browserOpenLogin(username, password);
      if (res.status === "ok") {
        notify(`Da them ${res.data.Username ?? "tai khoan"}`);
        await refreshAccounts();
        await get().refreshGroups();
        return true;
      }
      notify(`Loi dang nhap bang trinh duyet: ${res.error}`);
      return false;
    } catch (e) {
      notify(`Loi: ${String(e)}`);
      return false;
    } finally {
      set({ browserBusy: false });
    }
  },
}));
