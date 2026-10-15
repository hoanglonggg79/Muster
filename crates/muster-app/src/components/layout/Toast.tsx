import { useAppStore } from "../../store/useAppStore";

export function Toast() {
  const toast = useAppStore((s) => s.toast);
  if (!toast) return null;
  return (
    <aside aria-label="Notifications" className="toast" role="status">
      {toast}
    </aside>
  );
}
