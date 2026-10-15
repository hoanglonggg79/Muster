import { Clock, Gamepad2, Play, RefreshCw, Server, Star } from "lucide-react";
import { useEffect } from "react";
import { PRESET_GAMES } from "../../lib/accountUtils";
import { useAppStore } from "../../store/useAppStore";

export function GamesTab() {
  const placeId = useAppStore((s) => s.placeId);
  const setPlaceId = useAppStore((s) => s.setPlaceId);
  const jobId = useAppStore((s) => s.jobId);
  const setJobId = useAppStore((s) => s.setJobId);
  const accounts = useAppStore((s) => s.accounts);
  const selectedUserId = useAppStore((s) => s.selectedUserId);
  const launchAccount = useAppStore((s) => s.launchAccount);

  const servers = useAppStore((s) => s.servers);
  const serversCursor = useAppStore((s) => s.serversCursor);
  const serversLoading = useAppStore((s) => s.serversLoading);
  const refreshServers = useAppStore((s) => s.refreshServers);
  const pickServer = useAppStore((s) => s.pickServer);
  const joinServer = useAppStore((s) => s.joinServer);

  const favorites = useAppStore((s) => s.favorites);
  const recent = useAppStore((s) => s.recent);
  const refreshFavorites = useAppStore((s) => s.refreshFavorites);
  const refreshRecent = useAppStore((s) => s.refreshRecent);
  const pushRecent = useAppStore((s) => s.pushRecent);
  const toggleFavorite = useAppStore((s) => s.toggleFavorite);

  const activeUserId = selectedUserId ?? accounts[0]?.UserID ?? null;
  const numericPlaceId = Number(placeId.replace(/[^0-9]/g, ""));
  const isFavorite = favorites.some((item) => item.placeId === numericPlaceId);

  useEffect(() => {
    refreshFavorites();
    refreshRecent();
  }, [refreshFavorites, refreshRecent]);

  async function launchActive() {
    if (!activeUserId) return;
    const ok = await launchAccount(activeUserId);
    if (ok && numericPlaceId) {
      const known =
        PRESET_GAMES.find((game) => game.placeId === placeId)?.name ??
        favorites.find((item) => item.placeId === numericPlaceId)?.name ??
        recent.find((item) => item.placeId === numericPlaceId)?.name ??
        `Place ${numericPlaceId}`;
      await pushRecent(known, numericPlaceId);
    }
  }

  return (
    <div className="launch-bar">
      <div className="overview-banner__title">
        <Gamepad2 size={16} strokeWidth={1.75} style={{ color: "var(--accent)" }} />
        <span>Target Game Configuration</span>
      </div>

      <div className="launch-grid">
        <div className="input-group">
          <label className="input-label" htmlFor="games-place-id">
            Place ID (Roblox Experience ID)
          </label>
          <input
            id="games-place-id"
            className="input-field"
            type="text"
            value={placeId}
            onChange={(e) => setPlaceId(e.target.value)}
            placeholder="e.g. 4483381587"
          />
        </div>
        <div className="input-group">
          <label className="input-label" htmlFor="games-job-id">
            Job ID (Specific Server Instance)
          </label>
          <input
            id="games-job-id"
            className="input-field"
            type="text"
            value={jobId}
            onChange={(e) => setJobId(e.target.value)}
            placeholder="Leave empty for public join"
          />
        </div>
      </div>

      <div style={{ marginTop: "6px" }}>
        <span className="input-label" style={{ marginBottom: "8px", display: "block" }}>
          Quick Bookmarks
        </span>
        <div className="game-bookmarks">
          {PRESET_GAMES.map((game) => (
            <button
              key={game.placeId}
              type="button"
              className={`game-bookmark-chip ${placeId === game.placeId ? "active" : ""}`}
              onClick={() => setPlaceId(game.placeId)}
            >
              <span className="bookmark-title">{game.name}</span>
              <span className="bookmark-id">{game.placeId}</span>
            </button>
          ))}
        </div>
      </div>

      {favorites.length > 0 && (
        <div className="bookmark-row">
          <span className="input-label">
            <Star size={11} strokeWidth={1.75} /> Yêu thích
          </span>
          {favorites.map((game) => (
            <button
              key={`${game.placeId}-${game.privateServer ?? ""}`}
              type="button"
              className={`game-bookmark-chip ${numericPlaceId === game.placeId ? "active" : ""}`}
              onClick={() => {
                setPlaceId(String(game.placeId));
                setJobId(game.privateServer ?? "");
              }}
            >
              <span className="bookmark-title">{game.name}</span>
              <span className="bookmark-id">{game.placeId}</span>
            </button>
          ))}
        </div>
      )}

      {recent.length > 0 && (
        <div className="bookmark-row">
          <span className="input-label">
            <Clock size={11} strokeWidth={1.75} /> Gần đây
          </span>
          {recent.map((game) => (
            <button
              key={game.placeId}
              type="button"
              className={`game-bookmark-chip ${numericPlaceId === game.placeId ? "active" : ""}`}
              onClick={() => setPlaceId(String(game.placeId))}
            >
              <span className="bookmark-title">{game.name}</span>
              <span className="bookmark-id">{game.placeId}</span>
            </button>
          ))}
        </div>
      )}

      <div className="bookmark-row">
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          disabled={!numericPlaceId || serversLoading}
          onClick={() => refreshServers(placeId)}
        >
          <RefreshCw
            size={12}
            strokeWidth={1.75}
            style={serversLoading ? { animation: "spin 1s linear infinite" } : undefined}
          />
          <span>Xem danh sách server</span>
        </button>

        <button
          type="button"
          className="btn btn-secondary btn-sm"
          disabled={!numericPlaceId || serversLoading}
          onClick={() => pickServer(placeId, true)}
        >
          <Server size={12} strokeWidth={1.75} />
          <span>Chọn server vắng người</span>
        </button>

        <button
          type="button"
          className="btn btn-secondary btn-sm"
          disabled={!numericPlaceId || serversLoading}
          onClick={() => pickServer(placeId, false)}
        >
          <span>Chọn server ngẫu nhiên</span>
        </button>

        <button
          type="button"
          className={`btn btn-sm ${isFavorite ? "btn-primary" : "btn-secondary"}`}
          disabled={!numericPlaceId}
          onClick={() => toggleFavorite(`Place ${numericPlaceId}`, numericPlaceId)}
        >
          <Star size={12} strokeWidth={1.75} />
          <span>{isFavorite ? "Bỏ yêu thích" : "Thêm yêu thích"}</span>
        </button>
      </div>

      {servers.length > 0 && (
        <div className="server-panel">
          {servers.slice(0, 40).map((server) => (
            <div className="server-row" key={server.id}>
              <span className="server-row__id">{server.id}</span>
              <span className="server-row__players">
                {server.playing}/{server.maxPlayers}
              </span>
              <span className="server-row__meta">{server.fps ? `${server.fps} fps` : "—"}</span>
              <span className="server-row__meta">{server.ping ? `${server.ping} ms` : "—"}</span>
              <button
                type="button"
                className="btn btn-primary btn-sm"
                disabled={!activeUserId || server.playing === server.maxPlayers}
                onClick={() => {
                  if (activeUserId) joinServer(activeUserId, numericPlaceId, server.id);
                }}
              >
                <Play size={11} fill="currentColor" />
                <span>Join</span>
              </button>
            </div>
          ))}

          {serversCursor && (
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              style={{ alignSelf: "flex-start" }}
              disabled={serversLoading}
              onClick={() => refreshServers(placeId, serversCursor)}
            >
              Tải thêm server
            </button>
          )}
        </div>
      )}

      {accounts.length > 0 && (
        <div
          style={{
            display: "flex",
            justifyContent: "flex-end",
            marginTop: "10px",
          }}
        >
          <button type="button" className="btn btn-primary" onClick={launchActive}>
            <Play size={13} fill="currentColor" />
            <span>Launch Game Now</span>
          </button>
        </div>
      )}
    </div>
  );
}
