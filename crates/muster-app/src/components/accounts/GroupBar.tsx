import { FolderPlus, Layers, Pencil } from "lucide-react";
import { useEffect, useState } from "react";
import { useAppStore } from "../../store/useAppStore";

export function GroupBar() {
  const groups = useAppStore((s) => s.groups);
  const groupFilter = useAppStore((s) => s.groupFilter);
  const setGroupFilter = useAppStore((s) => s.setGroupFilter);
  const refreshGroups = useAppStore((s) => s.refreshGroups);
  const renameGroup = useAppStore((s) => s.renameGroup);
  const notify = useAppStore((s) => s.notify);

  const [renaming, setRenaming] = useState(false);
  const [draftName, setDraftName] = useState("");

  useEffect(() => {
    refreshGroups();
  }, [refreshGroups]);

  function startRename() {
    if (!groupFilter) {
      notify("Hãy chọn một nhóm trước");
      return;
    }
    setDraftName(groupFilter);
    setRenaming(true);
  }

  async function confirmRename() {
    const target = draftName.trim();
    if (!target) {
      notify("Tên nhóm không được để trống");
      return;
    }
    await renameGroup(groupFilter ?? "", target);
    setRenaming(false);
  }

  return (
    <div className="group-bar">
      <div className="group-bar__label">
        <Layers size={13} strokeWidth={1.75} color="var(--text-muted)" />
        <span>Nhóm</span>
      </div>

      <div className="group-chips">
        <button
          type="button"
          className={`group-chip ${groupFilter === null ? "active" : ""}`}
          onClick={() => setGroupFilter(null)}
        >
          Tất cả
        </button>

        {groups.map((group) => (
          <button
            key={group.name}
            type="button"
            className={`group-chip ${groupFilter === group.name ? "active" : ""}`}
            onClick={() => setGroupFilter(group.name)}
            title={group.name}
          >
            <span>{group.label}</span>
            <span className="group-chip__count">{group.count}</span>
          </button>
        ))}
      </div>

      {renaming ? (
        <div style={{ display: "flex", alignItems: "center", gap: "6px" }}>
          <input
            className="input-field"
            style={{ height: "26px", fontSize: "12px", width: "150px" }}
            value={draftName}
            onChange={(event) => setDraftName(event.target.value)}
            placeholder="1Main / 007 Bank / 9 Banana"
          />
          <button type="button" className="btn btn-primary btn-sm" onClick={confirmRename}>
            Lưu
          </button>
          <button
            type="button"
            className="btn btn-secondary btn-sm"
            onClick={() => setRenaming(false)}
          >
            Huỷ
          </button>
        </div>
      ) : (
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          onClick={startRename}
          title="Đổi tên nhóm đang chọn"
        >
          <Pencil size={12} strokeWidth={1.75} />
          <span>Đổi tên nhóm</span>
        </button>
      )}

      <div className="group-bar__hint">
        <FolderPlus size={12} strokeWidth={1.75} color="var(--text-muted)" />
        <span>Đặt tên nhóm có tiền tố số để sắp xếp: 1Main &lt; 007 Bank &lt; 9 Banana &lt; 10 Apple</span>
      </div>
    </div>
  );
}
