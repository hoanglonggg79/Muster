import type { Account } from "../bindings";

export type NavigationTab = "overview" | "accounts" | "games" | "settings";
export type LaunchPhase = "launching" | "done" | "error";

export interface PresetGame {
  name: string;
  placeId: string;
}

export const PRESET_GAMES: PresetGame[] = [
  { name: "Blox Fruits", placeId: "4483381587" },
  { name: "Blade Ball", placeId: "13772394625" },
  { name: "Deepwoken", placeId: "4111023553" },
  { name: "Pet Simulator 99", placeId: "1537690962" },
  { name: "Arsenal", placeId: "286090429" },
  { name: "Murder Mystery 2", placeId: "142823291" },
];

export function stripGroupPrefix(group: string): string {
  return group.replace(/^\d{1,3}\s?/, "");
}

export function groupLabel(acc: Account): string {
  return stripGroupPrefix(acc.Group?.trim() ? acc.Group : "Default");
}

export function presenceLabel(type: number): string {
  switch (type) {
    case 1:
      return "Online";
    case 2:
      return "In Game";
    case 3:
      return "In Studio";
    default:
      return "Offline";
  }
}

export function presenceClass(type: number): string {
  switch (type) {
    case 1:
      return "presence-badge presence-online";
    case 2:
      return "presence-badge presence-ingame";
    case 3:
      return "presence-badge presence-instudio";
    default:
      return "presence-badge presence-offline";
  }
}

export function savedPlaceId(acc: Account, fallbackPlaceId: string): string {
  const saved = acc.Fields?.SavedPlaceId;
  return saved?.trim() ? saved.trim() : fallbackPlaceId;
}

export function savedJobId(acc: Account, fallbackJobId: string): string | null {
  const saved = acc.Fields?.SavedJobId;
  if (saved?.trim()) return saved.trim();
  return fallbackJobId.trim() || null;
}
