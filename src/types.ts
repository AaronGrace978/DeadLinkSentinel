export type Site = {
  id: number;
  name: string;
  seed_url: string;
  allowlist_hosts: string;
  schedule_cron: string;
  mode: "deterministic" | "agentic";
  enabled: boolean;
  max_pages: number;
  max_links: number;
  concurrency: number;
  timeout_secs: number;
  triage_enabled: boolean;
  created_at: string;
};

export type SiteInput = Omit<Site, "id" | "created_at">;

export type Scan = {
  id: number;
  site_id: number;
  started_at: string;
  finished_at: string | null;
  status: string;
  pages_crawled: number;
  links_checked: number;
  broken_count: number;
  summary: string | null;
  error: string | null;
  site_name: string | null;
};

export type LinkResult = {
  id: number;
  scan_id: number;
  source_url: string;
  target_url: string;
  status_code: number | null;
  error: string | null;
  final_url: string | null;
  is_broken: boolean;
};

export type TriageNote = {
  id: number;
  scan_id: number;
  classification: string;
  grouping_key: string | null;
  draft_text: string;
  provider: string | null;
  model: string | null;
  created_at: string;
};

export type AppSettings = {
  status_bind_addr: string;
  status_bind_port: number;
  smtp_host: string;
  smtp_port: number;
  smtp_user: string;
  smtp_from: string;
  smtp_to: string;
  smtp_tls: string;
  default_llm_provider: string;
  default_llm_model: string;
  agent_max_steps: number;
  triage_after_scan: boolean;
  smtp_password_set: boolean;
};

export type ProviderConfig = {
  id: string;
  enabled: boolean;
  base_url: string | null;
  default_model: string | null;
  extra_json: string | null;
  api_key_set: boolean;
};

export type PublicStatus = {
  overall: string;
  generated_at: string;
  stale: boolean;
  sites: {
    name: string;
    seed_url: string;
    last_check: string | null;
    broken_count: number;
    pages_crawled: number;
    links_checked: number;
    health: string;
    summary: string | null;
  }[];
};

export type ScanProgress = {
  scan_id: number;
  site_id: number;
  pages_crawled: number;
  links_checked: number;
  broken_count: number;
  current_url: string;
  status: string;
};

export type View = "sites" | "scans" | "status" | "alerts" | "providers";
