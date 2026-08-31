import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  LinkResult,
  ProviderConfig,
  PublicStatus,
  Scan,
  Site,
  SiteInput,
  TriageNote,
} from "./types";

export const api = {
  listSites: () => invoke<Site[]>("list_sites"),
  createSite: (input: SiteInput) => invoke<Site>("create_site", { input }),
  updateSite: (id: number, input: SiteInput) =>
    invoke<Site>("update_site", { id, input }),
  deleteSite: (id: number) => invoke<void>("delete_site", { id }),
  runScanNow: (siteId: number) => invoke<number>("run_scan_now", { siteId }),
  cancelScan: (siteId: number) => invoke<boolean>("cancel_scan", { siteId }),
  listScans: (siteId?: number | null) =>
    invoke<Scan[]>("list_scans", { siteId: siteId ?? null, limit: 80 }),
  getScan: (id: number) => invoke<Scan>("get_scan", { id }),
  listScanLinks: (scanId: number, brokenOnly = false) =>
    invoke<LinkResult[]>("list_scan_links", { scanId, brokenOnly }),
  listTriageNotes: (scanId: number) =>
    invoke<TriageNote[]>("list_triage_notes", { scanId }),
  getSettings: () => invoke<AppSettings>("get_settings"),
  saveSettings: (patch: Partial<AppSettings> & { smtp_password?: string }) =>
    invoke<AppSettings>("save_settings", { patch }),
  sendTestEmail: () => invoke<string>("send_test_email"),
  getStatusUrl: () => invoke<string>("get_status_url"),
  getPublicStatus: () => invoke<PublicStatus>("get_public_status"),
  restartStatusServer: () => invoke<string>("restart_status_server"),
  listProviders: () => invoke<ProviderConfig[]>("list_providers"),
  saveProvider: (
    id: string,
    patch: {
      enabled?: boolean;
      base_url?: string;
      default_model?: string;
      api_key?: string;
    },
  ) => invoke<ProviderConfig>("save_provider", { id, patch }),
  testProvider: (id: string) => invoke<string>("test_provider", { id }),
  listProviderModels: (id: string) =>
    invoke<string[]>("list_provider_models", { id }),
};

export function errMsg(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) {
    return String((e as { message: unknown }).message);
  }
  return String(e);
}
