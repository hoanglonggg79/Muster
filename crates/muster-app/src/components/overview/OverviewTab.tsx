import {
  Coins,
  Compass,
  Layers,
  Play,
  Users,
  Zap,
} from "lucide-react";
import { useAppStore } from "../../store/useAppStore";

export function OverviewTab() {
  const accounts = useAppStore((s) => s.accounts);
  const multiRoblox = useAppStore((s) => s.multiRoblox);
  const unlockFps = useAppStore((s) => s.unlockFps);
  const maxFps = useAppStore((s) => s.maxFps);
  const placeId = useAppStore((s) => s.placeId);
  const setPlaceId = useAppStore((s) => s.setPlaceId);
  const jobId = useAppStore((s) => s.jobId);
  const setJobId = useAppStore((s) => s.setJobId);
  const selectedUserId = useAppStore((s) => s.selectedUserId);
  const setSelectedUserId = useAppStore((s) => s.setSelectedUserId);
  const launchAccount = useAppStore((s) => s.launchAccount);

  const totalRobux = accounts.reduce((acc, curr) => {
    const r = Number.parseInt(
      curr.Fields?.Robux?.replace(/[^0-9]/g, "") || "0",
      10
    );
    return acc + (Number.isNaN(r) ? 0 : r);
  }, 0);

  const activeUserId = selectedUserId ?? accounts[0]?.UserID ?? null;

  return (
    <div className="flex flex-col gap-5">
      <div className="overview-metrics">
        <div className="metric-card">
          <div className="metric-card__header">
            <span>Total Accounts</span>
            <Users size={15} strokeWidth={1.75} />
          </div>
          <div className="metric-card__val">{accounts.length}</div>
          <div className="metric-card__sub">Stored in encrypted vault</div>
        </div>

        <div className="metric-card">
          <div className="metric-card__header">
            <span>Total Robux</span>
            <Coins
              size={15}
              strokeWidth={1.75}
              style={{ color: "var(--gold-robux)" }}
            />
          </div>
          <div
            className="metric-card__val"
            style={{ color: "var(--gold-robux)" }}
          >
            {totalRobux.toLocaleString()}
          </div>
          <div className="metric-card__sub">Aggregate balance</div>
        </div>

        <div className="metric-card">
          <div className="metric-card__header">
            <span>Multi-Instance</span>
            <Layers size={15} strokeWidth={1.75} />
          </div>
          <div
            className="metric-card__val"
            style={{
              color: multiRoblox
                ? "var(--accent-text)"
                : "var(--text-muted)",
            }}
          >
            {multiRoblox ? "ACTIVE" : "STANDBY"}
          </div>
          <div className="metric-card__sub">Mutex patcher status</div>
        </div>

        <div className="metric-card">
          <div className="metric-card__header">
            <span>Target FPS</span>
            <Zap size={15} strokeWidth={1.75} />
          </div>
          <div className="metric-card__val">
            {unlockFps ? `${maxFps}` : "60"}
          </div>
          <div className="metric-card__sub">
            {unlockFps ? "Unlocked cap" : "Default engine limit"}
          </div>
        </div>
      </div>

      <div className="overview-banner">
        <div className="overview-banner__title">
          <Compass
            size={16}
            strokeWidth={1.75}
            style={{ color: "var(--accent)" }}
          />
          <span>Quick Launcher</span>
        </div>

        <div className="launch-grid">
          <div className="input-group">
            <label className="input-label" htmlFor="overview-place-id">
              Place ID (Game ID)
            </label>
            <input
              id="overview-place-id"
              className="input-field"
              type="text"
              value={placeId}
              onChange={(e) => setPlaceId(e.target.value)}
              placeholder="e.g. 4483381587"
            />
          </div>
          <div className="input-group">
            <label className="input-label" htmlFor="overview-job-id">
              Job ID (Server ID - Optional)
            </label>
            <input
              id="overview-job-id"
              className="input-field"
              type="text"
              value={jobId}
              onChange={(e) => setJobId(e.target.value)}
              placeholder="Leave empty for public matchmaking"
            />
          </div>
        </div>

        {accounts.length > 0 && (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              gap: "12px",
            }}
          >
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: "10px",
                flex: 1,
              }}
            >
              <label
                className="input-label"
                htmlFor="overview-select-account"
                style={{ whiteSpace: "nowrap" }}
              >
                Launch As:
              </label>
              <select
                id="overview-select-account"
                className="input-field"
                style={{ flex: 1, padding: "6px 10px" }}
                value={activeUserId ?? ""}
                onChange={(e) => setSelectedUserId(Number(e.target.value))}
              >
                {accounts.map((acc) => (
                  <option key={acc.UserID ?? 0} value={acc.UserID ?? 0}>
                    {acc.Fields?.DisplayName ||
                      acc.Alias ||
                      acc.Username ||
                      `ID: ${acc.UserID}`}{" "}
                    (@{acc.Username})
                  </option>
                ))}
              </select>
            </div>

            <button
              type="button"
              className="btn btn-primary"
              onClick={() => {
                if (activeUserId) launchAccount(activeUserId);
              }}
            >
              <Play size={13} fill="currentColor" />
              <span>Launch Instance</span>
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
