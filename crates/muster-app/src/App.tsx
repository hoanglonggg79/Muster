import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useEffect } from "react";
import { AccountsTab } from "./components/accounts/AccountsTab";
import { GamesTab } from "./components/games/GamesTab";
import { Sidebar } from "./components/layout/Sidebar";
import { Toast } from "./components/layout/Toast";
import { TopBar } from "./components/layout/TopBar";
import { AddAccountModal } from "./components/modals/AddAccountModal";
import { EditAccountModal } from "./components/modals/EditAccountModal";
import { ImportRamModal } from "./components/modals/ImportRamModal";
import { OverviewTab } from "./components/overview/OverviewTab";
import { SettingsTab } from "./components/settings/SettingsTab";
import { useAppStore } from "./store/useAppStore";

const queryClient = new QueryClient();

function MainView() {
  const activeTab = useAppStore((s) => s.activeTab);
  const accounts = useAppStore((s) => s.accounts);
  const refreshAccounts = useAppStore((s) => s.refreshAccounts);
  const refreshPresence = useAppStore((s) => s.refreshPresence);
  const initAppState = useAppStore((s) => s.initAppState);

  useEffect(() => {
    initAppState();
    refreshAccounts();
  }, [initAppState, refreshAccounts]);

  useEffect(() => {
    const timer = setInterval(() => {
      const validIds = accounts
        .map((acc) => acc.UserID ?? 0)
        .filter((id) => id > 0);
      refreshPresence(validIds);
    }, 120000);
    return () => clearInterval(timer);
  }, [accounts, refreshPresence]);

  return (
    <div className="app-shell">
      <Sidebar />

      <main className="workspace">
        <TopBar />
        <section className="workspace__content">
          {activeTab === "overview" && <OverviewTab />}
          {activeTab === "accounts" && <AccountsTab />}
          {activeTab === "games" && <GamesTab />}
          {activeTab === "settings" && <SettingsTab />}
        </section>
      </main>

      <AddAccountModal />
      <ImportRamModal />
      <EditAccountModal />
      <Toast />
    </div>
  );
}

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <MainView />
    </QueryClientProvider>
  );
}

export default App;
