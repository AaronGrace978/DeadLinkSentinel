import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, errMsg } from "./api";
import type {
  AppSettings,
  LinkResult,
  ProviderConfig,
  PublicStatus,
  Scan,
  ScanProgress,
  Site,
  SiteInput,
  TriageNote,
  View,
} from "./types";
import "./App.css";

const emptySite = (): SiteInput => ({
  name: "",
  seed_url: "https://",
  allowlist_hosts: "",
  schedule_cron: "0 0 * * *",
  mode: "deterministic",
  enabled: true,
  max_pages: 100,
  max_links: 500,
  concurrency: 8,
  timeout_secs: 15,
  triage_enabled: true,
});

export default function App() {
  const [view, setView] = useState<View>("sites");
  const [sites, setSites] = useState<Site[]>([]);
  const [scans, setScans] = useState<Scan[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [progress, setProgress] = useState<ScanProgress | null>(null);

  async function refresh() {
    try {
      const [s, sc] = await Promise.all([api.listSites(), api.listScans()]);
      setSites(s);
      setScans(sc);
    } catch (e) {
      setError(errMsg(e));
    }
  }

  useEffect(() => {
    refresh();
    let unlisten: (() => void) | undefined;
    listen<ScanProgress>("scan-progress", (ev) => {
      setProgress(ev.payload);
      if (
        ev.payload.status === "completed" ||
        ev.payload.status === "failed" ||
        ev.payload.status === "cancelled"
      ) {
        refresh();
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  const running = progress && progress.status === "running";

  return (
    <div className="shell">
      <aside>
        <div className="brand">
          <span className="mark" />
          <div>
            <strong>DeadLinkSentinel</strong>
            <small>docs link watchdog</small>
          </div>
        </div>
        <nav>
          {(
            [
              ["sites", "Sites"],
              ["scans", "Scans"],
              ["status", "Status page"],
              ["alerts", "Alerts"],
              ["providers", "Agent / providers"],
            ] as [View, string][]
          ).map(([id, label]) => (
            <button
              key={id}
              className={view === id ? "active" : ""}
              onClick={() => setView(id)}
            >
              {label}
            </button>
          ))}
        </nav>
        {running && (
          <div className="live">
            <span className="pulse" />
            Scanning… {progress.pages_crawled}p / {progress.links_checked}l
            <em>{progress.broken_count} broken</em>
          </div>
        )}
      </aside>
      <main>
        {error && (
          <div className="banner err" onClick={() => setError(null)}>
            {error}
          </div>
        )}
        {notice && (
          <div className="banner ok" onClick={() => setNotice(null)}>
            {notice}
          </div>
        )}
        {view === "sites" && (
          <SitesPage
            sites={sites}
            onChange={refresh}
            onError={setError}
            onNotice={setNotice}
            progress={progress}
          />
        )}
        {view === "scans" && (
          <ScansPage
            sites={sites}
            scans={scans}
            onChange={refresh}
            onError={setError}
            progress={progress}
          />
        )}
        {view === "status" && (
          <StatusPage onError={setError} onNotice={setNotice} />
        )}
        {view === "alerts" && (
          <AlertsPage onError={setError} onNotice={setNotice} />
        )}
        {view === "providers" && (
          <ProvidersPage onError={setError} onNotice={setNotice} />
        )}
      </main>
    </div>
  );
}

function SitesPage({
  sites,
  onChange,
  onError,
  onNotice,
  progress,
}: {
  sites: Site[];
  onChange: () => void;
  onError: (s: string) => void;
  onNotice: (s: string) => void;
  progress: ScanProgress | null;
}) {
  const [form, setForm] = useState<SiteInput>(emptySite());
  const [editing, setEditing] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);

  function load(site: Site) {
    setEditing(site.id);
    setForm({
      name: site.name,
      seed_url: site.seed_url,
      allowlist_hosts: site.allowlist_hosts,
      schedule_cron: site.schedule_cron,
      mode: site.mode,
      enabled: site.enabled,
      max_pages: site.max_pages,
      max_links: site.max_links,
      concurrency: site.concurrency,
      timeout_secs: site.timeout_secs,
      triage_enabled: site.triage_enabled,
    });
  }

  async function save() {
    setBusy(true);
    try {
      if (editing) await api.updateSite(editing, form);
      else await api.createSite(form);
      setForm(emptySite());
      setEditing(null);
      onChange();
      onNotice(editing ? "Site updated" : "Site added");
    } catch (e) {
      onError(errMsg(e));
    } finally {
      setBusy(false);
    }
  }

  async function run(id: number) {
    try {
      await api.runScanNow(id);
      onNotice("Scan started");
      onChange();
    } catch (e) {
      onError(errMsg(e));
    }
  }

  return (
    <section>
      <header className="page-h">
        <h1>Sites</h1>
        <p>Schedule crawls against docs hosts. Hard caps always apply.</p>
      </header>
      <div className="split">
        <div className="panel">
          <table>
            <thead>
              <tr>
                <th>Name</th>
                <th>Mode</th>
                <th>Cron</th>
                <th>On</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {sites.length === 0 && (
                <tr>
                  <td colSpan={5} className="muted">
                    No sites yet. Add a docs seed URL on the right.
                  </td>
                </tr>
              )}
              {sites.map((s) => (
                <tr key={s.id}>
                  <td>
                    <strong>{s.name}</strong>
                    <div className="muted tiny">{s.seed_url}</div>
                  </td>
                  <td>{s.mode}</td>
                  <td className="mono">{s.schedule_cron}</td>
                  <td>{s.enabled ? "yes" : "no"}</td>
                  <td className="actions">
                    <button onClick={() => load(s)}>Edit</button>
                    <button
                      className="primary"
                      disabled={progress?.site_id === s.id && progress.status === "running"}
                      onClick={() => run(s.id)}
                    >
                      Run now
                    </button>
                    <button
                      className="danger"
                      onClick={async () => {
                        if (!confirm(`Delete ${s.name}?`)) return;
                        try {
                          await api.deleteSite(s.id);
                          onChange();
                        } catch (e) {
                          onError(errMsg(e));
                        }
                      }}
                    >
                      Delete
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <form
          className="panel form"
          onSubmit={(e) => {
            e.preventDefault();
            save();
          }}
        >
          <h2>{editing ? "Edit site" : "New site"}</h2>
          <label>
            Name
            <input
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              required
            />
          </label>
          <label>
            Seed URL
            <input
              value={form.seed_url}
              onChange={(e) => setForm({ ...form, seed_url: e.target.value })}
              required
            />
          </label>
          <label>
            Allowlist hosts (comma-separated; seed host always included)
            <input
              value={form.allowlist_hosts}
              onChange={(e) =>
                setForm({ ...form, allowlist_hosts: e.target.value })
              }
              placeholder="docs.example.com, www.example.com"
            />
          </label>
          <label>
            Schedule (cron; 5-field or 6-field with seconds)
            <input
              className="mono"
              value={form.schedule_cron}
              onChange={(e) =>
                setForm({ ...form, schedule_cron: e.target.value })
              }
            />
          </label>
          <label>
            Mode
            <select
              value={form.mode}
              onChange={(e) =>
                setForm({
                  ...form,
                  mode: e.target.value as SiteInput["mode"],
                })
              }
            >
              <option value="deterministic">Deterministic crawl</option>
              <option value="agentic">Agentic crawl + drain</option>
            </select>
          </label>
          <div className="grid2">
            <label>
              Max pages
              <input
                type="number"
                value={form.max_pages}
                onChange={(e) =>
                  setForm({ ...form, max_pages: Number(e.target.value) })
                }
              />
            </label>
            <label>
              Max links
              <input
                type="number"
                value={form.max_links}
                onChange={(e) =>
                  setForm({ ...form, max_links: Number(e.target.value) })
                }
              />
            </label>
            <label>
              Concurrency
              <input
                type="number"
                value={form.concurrency}
                onChange={(e) =>
                  setForm({ ...form, concurrency: Number(e.target.value) })
                }
              />
            </label>
            <label>
              Timeout (s)
              <input
                type="number"
                value={form.timeout_secs}
                onChange={(e) =>
                  setForm({ ...form, timeout_secs: Number(e.target.value) })
                }
              />
            </label>
          </div>
          <label className="check">
            <input
              type="checkbox"
              checked={form.enabled}
              onChange={(e) => setForm({ ...form, enabled: e.target.checked })}
            />
            Enabled (scheduler)
          </label>
          <label className="check">
            <input
              type="checkbox"
              checked={form.triage_enabled}
              onChange={(e) =>
                setForm({ ...form, triage_enabled: e.target.checked })
              }
            />
            Post-scan AI triage
          </label>
          <div className="row">
            <button className="primary" disabled={busy} type="submit">
              {editing ? "Save" : "Add site"}
            </button>
            {editing && (
              <button
                type="button"
                onClick={() => {
                  setEditing(null);
                  setForm(emptySite());
                }}
              >
                Cancel
              </button>
            )}
          </div>
        </form>
      </div>
    </section>
  );
}

function ScansPage({
  sites,
  scans,
  onChange,
  onError,
  progress,
}: {
  sites: Site[];
  scans: Scan[];
  onChange: () => void;
  onError: (s: string) => void;
  progress: ScanProgress | null;
}) {
  const [selected, setSelected] = useState<number | null>(scans[0]?.id ?? null);
  const [links, setLinks] = useState<LinkResult[]>([]);
  const [notes, setNotes] = useState<TriageNote[]>([]);
  const [brokenOnly, setBrokenOnly] = useState(true);
  const [filterSite, setFilterSite] = useState<number | "all">("all");

  const shown = useMemo(
    () =>
      filterSite === "all"
        ? scans
        : scans.filter((s) => s.site_id === filterSite),
    [scans, filterSite],
  );

  useEffect(() => {
    if (!selected && shown[0]) setSelected(shown[0].id);
  }, [shown, selected]);

  useEffect(() => {
    if (!selected) return;
    api
      .listScanLinks(selected, brokenOnly)
      .then(setLinks)
      .catch((e) => onError(errMsg(e)));
    api
      .listTriageNotes(selected)
      .then(setNotes)
      .catch((e) => onError(errMsg(e)));
  }, [selected, brokenOnly, onError, progress?.status]);

  return (
    <section>
      <header className="page-h">
        <h1>Scans</h1>
        <p>History, broken links, and agent triage notes.</p>
      </header>
      <div className="toolbar">
        <select
          value={filterSite}
          onChange={(e) =>
            setFilterSite(
              e.target.value === "all" ? "all" : Number(e.target.value),
            )
          }
        >
          <option value="all">All sites</option>
          {sites.map((s) => (
            <option key={s.id} value={s.id}>
              {s.name}
            </option>
          ))}
        </select>
        <label className="check">
          <input
            type="checkbox"
            checked={brokenOnly}
            onChange={(e) => setBrokenOnly(e.target.checked)}
          />
          Broken only
        </label>
        {progress?.status === "running" && (
          <button onClick={() => api.cancelScan(progress.site_id).then(onChange)}>
            Cancel running scan
          </button>
        )}
      </div>
      <div className="split">
        <div className="panel">
          <table>
            <thead>
              <tr>
                <th>When</th>
                <th>Site</th>
                <th>Status</th>
                <th>Broken</th>
              </tr>
            </thead>
            <tbody>
              {shown.length === 0 && (
                <tr>
                  <td colSpan={4} className="muted">
                    No scans yet.
                  </td>
                </tr>
              )}
              {shown.map((s) => (
                <tr
                  key={s.id}
                  className={selected === s.id ? "sel" : ""}
                  onClick={() => setSelected(s.id)}
                >
                  <td className="mono tiny">{s.started_at}</td>
                  <td>{s.site_name ?? s.site_id}</td>
                  <td>
                    <span className={`pill ${s.status}`}>{s.status}</span>
                  </td>
                  <td>{s.broken_count}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div className="panel">
          {progress?.status === "running" && progress.scan_id === selected && (
            <p className="muted">
              Live: {progress.current_url} · {progress.pages_crawled} pages ·{" "}
              {progress.links_checked} links
            </p>
          )}
          <h2>Links</h2>
          <div className="scroll">
            <table>
              <thead>
                <tr>
                  <th>Target</th>
                  <th>Code</th>
                  <th>From</th>
                </tr>
              </thead>
              <tbody>
                {links.map((l) => (
                  <tr key={l.id} className={l.is_broken ? "broken" : ""}>
                    <td className="tiny wrap">{l.target_url}</td>
                    <td>{l.status_code ?? l.error ?? "err"}</td>
                    <td className="tiny muted wrap">{l.source_url}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <h2>Triage</h2>
          {notes.length === 0 ? (
            <p className="muted">No triage notes for this scan.</p>
          ) : (
            notes.map((n) => (
              <article key={n.id} className="note">
                <header>
                  <span className="pill">{n.classification}</span>
                  {n.grouping_key && <small>{n.grouping_key}</small>}
                </header>
                <p>{n.draft_text}</p>
              </article>
            ))
          )}
        </div>
      </div>
    </section>
  );
}

function StatusPage({
  onError,
  onNotice,
}: {
  onError: (s: string) => void;
  onNotice: (s: string) => void;
}) {
  const [url, setUrl] = useState("");
  const [status, setStatus] = useState<PublicStatus | null>(null);
  const [addr, setAddr] = useState("127.0.0.1");
  const [port, setPort] = useState(8787);

  async function load() {
    try {
      const [u, s, settings] = await Promise.all([
        api.getStatusUrl(),
        api.getPublicStatus(),
        api.getSettings(),
      ]);
      setUrl(u);
      setStatus(s);
      setAddr(settings.status_bind_addr);
      setPort(settings.status_bind_port);
    } catch (e) {
      onError(errMsg(e));
    }
  }

  useEffect(() => {
    load();
  }, []);

  return (
    <section>
      <header className="page-h">
        <h1>Public status page</h1>
        <p>
          Served only while this app is running. Binding to 0.0.0.0 exposes it
          on your LAN with no authentication.
        </p>
      </header>
      <div className="panel form">
        <p>
          Live URL:{" "}
          <a href={url} onClick={(e) => e.preventDefault()}>
            {url || "not running"}
          </a>
        </p>
        <div className="row">
          <button
            onClick={() => {
              navigator.clipboard.writeText(url);
              onNotice("Copied");
            }}
          >
            Copy URL
          </button>
          <button
            className="primary"
            onClick={() => url && openUrl(url).catch((e) => onError(errMsg(e)))}
          >
            Open
          </button>
        </div>
        <div className="grid2">
          <label>
            Bind address
            <input value={addr} onChange={(e) => setAddr(e.target.value)} />
          </label>
          <label>
            Port
            <input
              type="number"
              value={port}
              onChange={(e) => setPort(Number(e.target.value))}
            />
          </label>
        </div>
        <div className="row">
          <button
            onClick={async () => {
              try {
                await api.saveSettings({
                  status_bind_addr: addr,
                  status_bind_port: port,
                });
                const u = await api.restartStatusServer();
                setUrl(u);
                onNotice(`Status server at ${u}`);
                load();
              } catch (e) {
                onError(errMsg(e));
              }
            }}
          >
            Save & restart server
          </button>
        </div>
      </div>
      {status && (
        <div className="cards">
          <div className={`health ${status.overall}`}>
            Overall: {status.overall}
            {status.stale ? " (stale — no completed scans)" : ""}
          </div>
          {status.sites.map((s) => (
            <article key={s.name} className={`card ${s.health}`}>
              <header>
                <h2>{s.name}</h2>
                <span className="pill">{s.health}</span>
              </header>
              <p className="muted tiny">{s.seed_url}</p>
              <p>
                {s.broken_count} broken · {s.links_checked} checked · last{" "}
                {s.last_check}
              </p>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}

function AlertsPage({
  onError,
  onNotice,
}: {
  onError: (s: string) => void;
  onNotice: (s: string) => void;
}) {
  const [s, setS] = useState<AppSettings | null>(null);
  const [password, setPassword] = useState("");

  useEffect(() => {
    api.getSettings().then(setS).catch((e) => onError(errMsg(e)));
  }, [onError]);

  if (!s) return <p className="muted">Loading…</p>;

  return (
    <section>
      <header className="page-h">
        <h1>Email alerts</h1>
        <p>
          SMTP is used only when a scan finds new or regressed broken links.
        </p>
      </header>
      <form
        className="panel form narrow"
        onSubmit={async (e) => {
          e.preventDefault();
          try {
            const next = await api.saveSettings({
              ...s,
              smtp_password: password || undefined,
            });
            setS(next);
            setPassword("");
            onNotice("SMTP settings saved");
          } catch (err) {
            onError(errMsg(err));
          }
        }}
      >
        <label>
          Host
          <input
            value={s.smtp_host}
            onChange={(e) => setS({ ...s, smtp_host: e.target.value })}
          />
        </label>
        <div className="grid2">
          <label>
            Port
            <input
              type="number"
              value={s.smtp_port}
              onChange={(e) =>
                setS({ ...s, smtp_port: Number(e.target.value) })
              }
            />
          </label>
          <label>
            TLS
            <select
              value={s.smtp_tls}
              onChange={(e) => setS({ ...s, smtp_tls: e.target.value })}
            >
              <option value="starttls">STARTTLS</option>
              <option value="tls">TLS wrapper</option>
              <option value="none">None</option>
            </select>
          </label>
        </div>
        <label>
          Username
          <input
            value={s.smtp_user}
            onChange={(e) => setS({ ...s, smtp_user: e.target.value })}
          />
        </label>
        <label>
          Password {s.smtp_password_set ? "(saved)" : ""}
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder={s.smtp_password_set ? "unchanged" : ""}
          />
        </label>
        <label>
          From
          <input
            value={s.smtp_from}
            onChange={(e) => setS({ ...s, smtp_from: e.target.value })}
            placeholder="alerts@example.com"
          />
        </label>
        <label>
          To
          <input
            value={s.smtp_to}
            onChange={(e) => setS({ ...s, smtp_to: e.target.value })}
            placeholder="you@example.com"
          />
        </label>
        <div className="row">
          <button className="primary" type="submit">
            Save
          </button>
          <button
            type="button"
            onClick={async () => {
              try {
                onNotice(await api.sendTestEmail());
              } catch (e) {
                onError(errMsg(e));
              }
            }}
          >
            Send test email
          </button>
        </div>
      </form>
    </section>
  );
}

function ProvidersPage({
  onError,
  onNotice,
}: {
  onError: (s: string) => void;
  onNotice: (s: string) => void;
}) {
  const [providers, setProviders] = useState<ProviderConfig[]>([]);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [keys, setKeys] = useState<Record<string, string>>({});
  const [models, setModels] = useState<Record<string, string[]>>({});

  async function load() {
    try {
      const [p, s] = await Promise.all([
        api.listProviders(),
        api.getSettings(),
      ]);
      setProviders(p);
      setSettings(s);
    } catch (e) {
      onError(errMsg(e));
    }
  }

  useEffect(() => {
    load();
  }, []);

  if (!settings) return <p className="muted">Loading…</p>;

  const labels: Record<string, string> = {
    "ollama-cloud": "Ollama Cloud",
    openai: "OpenAI",
    anthropic: "Anthropic",
    "openai-compat": "OpenAI-compatible",
  };

  return (
    <section>
      <header className="page-h">
        <h1>Agent / providers</h1>
        <p>
          Ollama Cloud is the default. Keys go in the OS keyring when available.
        </p>
      </header>
      <div className="panel form narrow">
        <label>
          Default provider
          <select
            value={settings.default_llm_provider}
            onChange={(e) =>
              setSettings({ ...settings, default_llm_provider: e.target.value })
            }
          >
            {providers.map((p) => (
              <option key={p.id} value={p.id}>
                {labels[p.id] ?? p.id}
              </option>
            ))}
          </select>
        </label>
        <label>
          Default model
          <input
            value={settings.default_llm_model}
            onChange={(e) =>
              setSettings({ ...settings, default_llm_model: e.target.value })
            }
          />
        </label>
        <label>
          Agent max steps
          <input
            type="number"
            value={settings.agent_max_steps}
            onChange={(e) =>
              setSettings({
                ...settings,
                agent_max_steps: Number(e.target.value),
              })
            }
          />
        </label>
        <button
          className="primary"
          onClick={async () => {
            try {
              const next = await api.saveSettings({
                default_llm_provider: settings.default_llm_provider,
                default_llm_model: settings.default_llm_model,
                agent_max_steps: settings.agent_max_steps,
              });
              setSettings(next);
              onNotice("Agent defaults saved");
            } catch (e) {
              onError(errMsg(e));
            }
          }}
        >
          Save defaults
        </button>
      </div>
      <div className="cards">
        {providers.map((p) => (
          <article key={p.id} className="card">
            <header>
              <h2>{labels[p.id] ?? p.id}</h2>
              <label className="check">
                <input
                  type="checkbox"
                  checked={p.enabled}
                  onChange={async (e) => {
                    try {
                      await api.saveProvider(p.id, { enabled: e.target.checked });
                      load();
                    } catch (err) {
                      onError(errMsg(err));
                    }
                  }}
                />
                Enabled
              </label>
            </header>
            <label>
              Base URL
              <input
                defaultValue={p.base_url ?? ""}
                onBlur={async (e) => {
                  try {
                    await api.saveProvider(p.id, { base_url: e.target.value });
                  } catch (err) {
                    onError(errMsg(err));
                  }
                }}
              />
            </label>
            <label>
              Default model
              <input
                defaultValue={p.default_model ?? ""}
                onBlur={async (e) => {
                  try {
                    await api.saveProvider(p.id, {
                      default_model: e.target.value,
                    });
                  } catch (err) {
                    onError(errMsg(err));
                  }
                }}
              />
            </label>
            <label>
              API key {p.api_key_set ? "(saved)" : ""}
              <input
                type="password"
                value={keys[p.id] ?? ""}
                placeholder={p.api_key_set ? "unchanged" : "paste key"}
                onChange={(e) => setKeys({ ...keys, [p.id]: e.target.value })}
              />
            </label>
            <div className="row">
              <button
                onClick={async () => {
                  try {
                    if (keys[p.id]) {
                      await api.saveProvider(p.id, { api_key: keys[p.id] });
                      setKeys({ ...keys, [p.id]: "" });
                    }
                    onNotice(await api.testProvider(p.id));
                    load();
                  } catch (e) {
                    onError(errMsg(e));
                  }
                }}
              >
                Test connection
              </button>
              <button
                onClick={async () => {
                  try {
                    const list = await api.listProviderModels(p.id);
                    setModels({ ...models, [p.id]: list });
                  } catch (e) {
                    onError(errMsg(e));
                  }
                }}
              >
                List models
              </button>
            </div>
            {models[p.id] && (
              <p className="muted tiny wrap">{models[p.id].join(", ")}</p>
            )}
          </article>
        ))}
      </div>
    </section>
  );
}
