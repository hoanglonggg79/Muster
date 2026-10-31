import { Clock, Gamepad2, Play, RefreshCw, Search, Server, Star } from "lucide-react";
import { useEffect, useState } from "react";
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
  const notify = useAppStore((s) => s.notify);

  const games = useAppStore((s) => s.games);
  const gameThumbnails = useAppStore((s) => s.gameThumbnails);
  const refreshGames = useAppStore((s) => s.refreshGames);

  const activeUserId = selectedUserId ?? accounts[0]?.UserID ?? null;
  const numericPlaceId = Number(placeId.replace(/[^0-9]/g, ""));
  const isFavorite = favorites.some((item) => item.placeId === numericPlaceId);
  const activeGame = games[numericPlaceId];
  const activeThumbnail = gameThumbnails[numericPlaceId];
  const activeGameName = activeGame?.name ?? `Place ${numericPlaceId}`;

  const [gameSearch, setGameSearch] = useState("");
  const searchLower = gameSearch.trim().toLowerCase();
  const filteredPresets = PRESET_GAMES.filter((game) => {
    if (!searchLower) return true;
    return (
      game.name.toLowerCase().includes(searchLower) ||
      game.placeId.includes(searchLower)
    );
  });

  function handleGameSearchSubmit(e: React.FormEvent<HTMLFormElement>) {
    e.preventDefault();
    const q = gameSearch.trim();
    if (!q) return;
    const digits = q.replace(/[^0-9]/g, "");
    if (digits && digits === q) {
      setPlaceId(q);
      refreshGames([Number(q)]);
      return;
    }
    const match = PRESET_GAMES.find((game) => game.name.toLowerCase() === q);
    if (match) {
      setPlaceId(match.placeId);
      refreshGames([Number(match.placeId)]);
    } else {
      notify(`Không tìm thấy game "${q}"`);
    }
  }

  useEffect(() => {
    refreshFavorites();
    refreshRecent();
    refreshGames(PRESET_GAMES.map((game) => Number(game.placeId)));
  }, [refreshFavorites, refreshRecent, refreshGames]);

  async function launchActive() {
    if (!activeUserId) return;
    const ok = await launchAccount(activeUserId);
    if (ok && numericPlaceId) {
      const known =
        games[numericPlaceId]?.name ??
        PRESET_GAMES.find((game) => game.placeId === placeId)?.name ??
        favorites.find((item) => item.placeId === numericPlaceId)?.name ??
        recent.find((item) => item.placeId === numericPlaceId)?.name ??
        `Place ${numericPlaceId}`;
      await pushRecent(known, numericPlaceId);
    }
  }

  function gameChip(name: string, placeIdValue: string, active: boolean) {
    const pid = Number(placeIdValue.replace(/[^0-9]/g, ""));
    const game = games[pid];
    const thumbnail = gameThumbnails[pid];
    const resolvedName = game?.name ?? name;
    return (
      <button
        key={placeIdValue}
        type="button"
        className={`game-bookmark-chip ${active ? "active" : ""}`}
        onClick={() => setPlaceId(placeIdValue)}
      >
        {thumbnail ? (
          <img
            className="bookmark-thumb"
            src={thumbnail}
            alt={resolvedName}
            loading="lazy"
            decoding="async"
          />
        ) : null}
        <span className="bookmark-title">{resolvedName}</span>
        <span className="bookmark-id">{placeIdValue}</span>
      </button>
    );
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
            onBlur={() => {
              const pid = Number(placeId.replace(/[^0-9]/g, ""));
              if (pid && !Number.isNaN(pid)) refreshGames([pid]);
            }}
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

      {numericPlaceId && (
        <div className="game-bookmarks" style={{ gridTemplateColumns: "1fr", gap: 0, marginBottom: 8 }}>
          <div className="game-bookmark-chip" style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between", padding: "10px 12px" }}>
            {activeThumbnail ? (
              <img
                className="bookmark-thumb"
                src={activeThumbnail}
                alt={activeGameName}
                style={{ width: 56, height: 56, marginRight: 10, flexShrink: 0 }}
                loading="lazy"
                decoding="async"
              />
            ) : null}
            <div>
              <span className="bookmark-title" style={{ display: "block", fontSize: 13 }}>{activeGameName}</span>
              <span className="bookmark-id" style={{ display: "block", fontSize: 11 }}>{numericPlaceId}</span>
            </div>
          </div>
        </div>
      )}

      <div style={{ marginTop: "6px" }}>
        <form onSubmit={handleGameSearchSubmit} style={{ display: "flex", gap: 8, marginBottom: 10 }}>
          <input
            id="games-search"
            className="input-field"
            value={gameSearch}
            onChange={(e) => setGameSearch(e.target.value)}
            placeholder="Tên game hoặc Place ID (vd. Blox Fruits / 4483381587)"
            style={{ flex: 1 }}
          />
          <button type="submit" className="btn btn-primary btn-sm">
            <Search size={13} fill="currentColor" />
          </button>
        </form>
        <span className="input-label" style={{ marginBottom: "8px", display: "block" }}>
          Quick Bookmarks
        </span>
        <div className="game-bookmarks">
          {filteredPresets.map((game) => (
            gameChip(game.name, game.placeId, placeId === game.placeId)
          ))}
        </div>
        {searchLower && filteredPresets.length === 0 && (
          <p style={{ marginTop: 6, fontSize: 12, color: "var(--text-muted)" }}>
            Không có game nào khớp
          </p>
        )}
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
