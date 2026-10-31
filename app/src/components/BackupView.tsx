import { open, save } from "@tauri-apps/plugin-dialog";
import { motion } from "motion/react";
import {
  Archive,
  Check,
  Download,
  FileArchive,
  History,
  Info,
  LoaderCircle,
  PackageOpen,
  ShieldAlert,
  Trash2,
  Upload,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "../design/primitives/Toast";
import { spring, staggerDelay } from "../design/springs";
import { formatBytes } from "../lib/fontLoader";
import { useT, type TKey } from "../lib/i18n";
import { ipc, type AuditEntry, type ImportPlan, type ImportResult } from "../lib/ipc";
import { rowsFor } from "../lib/exporter";
import { familiesFor, useFontStore, type Family } from "../state/fontStore";

const DAY = 86400;

function stamp(): string {
  const d = new Date();
  return `${d.getFullYear()}${String(d.getMonth() + 1).padStart(2, "0")}${String(d.getDate()).padStart(2, "0")}`;
}

function clock(sec: number): string {
  const d = new Date(sec * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function ago(sec: number, t: (k: TKey, v?: Record<string, string | number>) => string): string {
  if (!sec) return "";
  const diff = Math.max(0, Math.floor(Date.now() / 1000) - sec);
  if (diff < 60) return t("usage.justNow");
  if (diff < 3600) return t("usage.minutesAgo", { count: Math.floor(diff / 60) });
  if (diff < DAY) return t("usage.hoursAgo", { count: Math.floor(diff / 3600) });
  if (diff < 30 * DAY) return t("usage.daysAgo", { count: Math.floor(diff / DAY) });
  return clock(sec);
}

function baseName(path: string): string {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

function csvCell(raw: string): string {
  return `"${raw.replace(/"/g, '""')}"`;
}

/** 恢复结果 → 一句话里的六个数字。 */
function summaryVars(r: ImportResult) {
  return {
    tags: r.tags,
    collections: r.collections,
    favorites: r.favorites,
    ratings: r.ratings,
    aliases: r.aliases,
    notes: r.notes,
  };
}

/** 日志里那条主文案：每个 kind 一句完整的话，数量从后端来。 */
function auditLabel(entry: AuditEntry, t: (k: TKey, v?: Record<string, string | number>) => string) {
  const key = `audit.kind.${entry.kind}` as TKey;
  return t(key, { count: entry.count });
}

/** 第二行灰字：补一点「哪个字体、哪个目录、哪种方式」。 */
function auditDetail(entry: AuditEntry, t: (k: TKey, v?: Record<string, string | number>) => string) {
  const d = entry.detail ?? {};
  const text = (k: string) => (typeof d[k] === "string" ? (d[k] as string) : "");
  const num = (k: string) => (typeof d[k] === "number" ? (d[k] as number) : 0);
  switch (entry.kind) {
    case "install":
      return text("mode") === "link" ? t("audit.modeLink") : t("audit.modeCopy");
    case "uninstall":
      return [text("family"), baseName(text("path"))].filter(Boolean).join(" · ");
    case "organize": {
      const mode =
        text("mode") === "move"
          ? t("audit.modeMove")
          : text("mode") === "link"
            ? t("audit.modeLink")
            : t("audit.modeCopy");
      return [mode, text("dest")].filter(Boolean).join(" · ");
    }
    case "bundle-in":
      return text("mode") === "replace" ? t("backup.mode.replace") : t("backup.mode.merge");
    case "bundle-out": {
      const parts = [`${num("files")} ${t("backup.countFonts")}`];
      if (d.withUsage) parts.push(t("backup.withUsage"));
      return parts.join(" · ");
    }
    default:
      return "";
  }
}

function Toggle({
  checked,
  onChange,
  label,
  hint,
  icon,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  label: string;
  hint: string;
  icon: React.ReactNode;
}) {
  return (
    <label className="bk-toggle">
      <span className="bk-toggle-icon">{icon}</span>
      <span className="bk-toggle-text">
        <span className="bk-toggle-label">{label}</span>
        <span className="bk-toggle-hint">{hint}</span>
      </span>
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="bk-check"
      />
    </label>
  );
}

function CountChip({ value, label }: { value: number; label: string }) {
  return (
    <span className={`bk-chip ${value === 0 ? "is-empty" : ""}`}>
      <span className="bk-chip-value tabular">{value}</span>
      {label}
    </span>
  );
}

export function BackupView() {
  const t = useT();
  const fonts = useFontStore((s) => s.fonts);
  const tags = useFontStore((s) => s.tags);
  const collections = useFontStore((s) => s.collections);
  const favorites = useFontStore((s) => s.favorites);
  const ratings = useFontStore((s) => s.ratings);
  const aliases = useFontStore((s) => s.aliases);
  const notes = useFontStore((s) => s.notes);
  const reloadAssets = useFontStore((s) => s.reloadAssets);
  const rescan = useFontStore((s) => s.rescan);

  const families = useMemo(() => [...familiesFor(fonts, tags).values()], [fonts, tags]);

  const assets = {
    tags: Object.keys(tags).length,
    collections: Object.keys(collections).length,
    favorites: favorites.length,
    ratings: Object.keys(ratings).length,
    aliases: Object.keys(aliases).length,
    notes: Object.keys(notes).length,
  };

  const [withFiles, setWithFiles] = useState(false);
  const [withUsage, setWithUsage] = useState(true);
  const [exporting, setExporting] = useState(false);

  const [path, setPath] = useState<string | null>(null);
  const [plan, setPlan] = useState<ImportPlan | null>(null);
  const [inspecting, setInspecting] = useState(false);
  const [mode, setMode] = useState<"merge" | "replace">("merge");
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<ImportResult | null>(null);

  const [audit, setAudit] = useState<AuditEntry[]>([]);
  const [auditLoading, setAuditLoading] = useState(true);
  const [confirmClear, setConfirmClear] = useState(false);

  const loadAudit = useCallback(async () => {
    try {
      setAudit(await ipc.auditLog(120));
    } catch {
      setAudit([]);
    } finally {
      setAuditLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadAudit();
  }, [loadAudit]);

  const doExport = async () => {
    const dest = await save({
      defaultPath: `Fontora-${stamp()}.zip`,
      filters: [{ name: t("backup.pickFilter"), extensions: ["zip"] }],
    });
    if (!dest) return;
    setExporting(true);
    try {
      const out = await ipc.bundleExport(
        dest,
        rowsFor(families.map((f: Family) => f.name)),
        withFiles,
        withUsage,
      );
      toast.success(
        t("backup.toastExported"),
        out.files > 0
          ? t("backup.exportDone", { files: out.files, size: formatBytes(out.bytes) })
          : t("backup.exportDoneNoFiles", { size: formatBytes(out.bytes) }),
        "tick",
      );
      void loadAudit();
    } catch (e) {
      toast.error(t("backup.toastExportFailed"), String(e));
    } finally {
      setExporting(false);
    }
  };

  const pick = async () => {
    const picked = await open({
      multiple: false,
      title: t("backup.pickTitle"),
      filters: [{ name: t("backup.pickFilter"), extensions: ["zip"] }],
    });
    if (typeof picked !== "string") return;
    setInspecting(true);
    setResult(null);
    setPath(picked);
    setPlan(null);
    try {
      const next = await ipc.bundleInspect(
        picked,
        families.map((f: Family) => f.name),
      );
      setPlan(next);
      setMode("merge");
    } catch (e) {
      setPath(null);
      toast.error(t("backup.toastPickFailed"), String(e));
    } finally {
      setInspecting(false);
    }
  };

  const run = async () => {
    if (!path || !plan) return;
    setRunning(true);
    try {
      const res = await ipc.bundleImport(path, mode);
      setResult(res);
      setPath(null);
      setPlan(null);
      await reloadAssets();
      if (res.fontsWritten > 0) void rescan();
      void loadAudit();
      toast.success(t("backup.toastImported"), t("backup.doneSummary", summaryVars(res)), "tick");
    } catch (e) {
      toast.error(t("backup.toastImportFailed"), String(e));
    } finally {
      setRunning(false);
    }
  };

  const exportAudit = async () => {
    const dest = await save({
      defaultPath: `Fontora-log-${stamp()}.csv`,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    if (!dest) return;
    const header = ["time", "action", "count", "detail"].map(csvCell).join(",");
    const lines = audit.map((entry) =>
      [
        csvCell(clock(entry.at)),
        csvCell(auditLabel(entry, t)),
        String(entry.count),
        csvCell(auditDetail(entry, t)),
      ].join(","),
    );
    try {
      await ipc.writeTextFile(dest, `\ufeff${[header, ...lines].join("\r\n")}`);
      toast.success(t("toast.exported"), baseName(dest), "tick");
    } catch (e) {
      toast.error(t("toast.exportFailed"), String(e));
    }
  };

  const clearAudit = async () => {
    if (!confirmClear) {
      setConfirmClear(true);
      return;
    }
    setConfirmClear(false);
    try {
      await ipc.auditClear();
      setAudit([]);
      toast.success(t("audit.cleared"), undefined, "tick");
    } catch (e) {
      toast.error(t("toast.couldntDelete"), String(e));
    }
  };

  return (
    <div className="ref-view">
      <div className="ref-inner">
        <header className="ref-head">
          <div className="ref-head-text">
            <h1 className="ref-title">{t("backup.title")}</h1>
            <p className="ref-sub">{t("backup.subtitle")}</p>
          </div>
        </header>

        <div className="ref-body">
          <div className="ref-stats">
            <div className="ref-stat">
              <span className="ref-stat-value tabular">{families.length}</span>
              <span className="ref-stat-label">{t("usage.statFamilies")}</span>
            </div>
            <div className="ref-stat">
              <span className="ref-stat-value tabular">{assets.tags}</span>
              <span className="ref-stat-label">{t("backup.statTags")}</span>
            </div>
            <div className="ref-stat">
              <span className="ref-stat-value tabular">{assets.collections}</span>
              <span className="ref-stat-label">{t("backup.statCollections")}</span>
            </div>
            <div className="ref-stat">
              <span className="ref-stat-value tabular">{assets.favorites}</span>
              <span className="ref-stat-label">{t("backup.statFavorites")}</span>
            </div>
            <div className="ref-stat">
              <span className="ref-stat-value tabular">{assets.ratings}</span>
              <span className="ref-stat-label">{t("backup.statRatings")}</span>
            </div>
            <div className="ref-stat">
              <span className="ref-stat-value tabular">{assets.aliases}</span>
              <span className="ref-stat-label">{t("backup.statAliases")}</span>
            </div>
            <div className="ref-stat">
              <span className="ref-stat-value tabular">{assets.notes}</span>
              <span className="ref-stat-label">{t("backup.statNotes")}</span>
            </div>
          </div>

          <section className="ref-section">
            <div className="ref-section-head">
              <Archive size={13} strokeWidth={1.8} />
              {t("backup.exportTitle")}
            </div>
            <p className="us-hint">{t("backup.exportHint")}</p>

            <div className="bk-card">
              <div className="bk-toggles">
                <Toggle
                  checked={withFiles}
                  onChange={setWithFiles}
                  label={t("backup.withFiles")}
                  hint={t("backup.withFilesSub")}
                  icon={<FileArchive size={14} strokeWidth={1.7} />}
                />
                <Toggle
                  checked={withUsage}
                  onChange={setWithUsage}
                  label={t("backup.withUsage")}
                  hint={t("backup.withUsageSub")}
                  icon={<History size={14} strokeWidth={1.7} />}
                />
              </div>
              <div className="bk-card-foot">
                <span className="bk-foot-note">
                  {t("backup.footFaces", { count: families.length })}
                </span>
                <button className="ref-btn primary" onClick={() => void doExport()} disabled={exporting}>
                  {exporting ? (
                    <LoaderCircle size={13} strokeWidth={1.8} className="bk-spin" />
                  ) : (
                    <Download size={13} strokeWidth={1.8} />
                  )}
                  {exporting ? t("backup.exporting") : t("backup.export")}
                </button>
              </div>
            </div>
          </section>

          <section className="ref-section">
            <div className="ref-section-head">
              <Upload size={13} strokeWidth={1.8} />
              {t("backup.importTitle")}
            </div>
            <p className="us-hint">{t("backup.importHint")}</p>

            {!plan && !inspecting && !result && (
              <div className="bk-empty">
                <PackageOpen size={20} strokeWidth={1.4} />
                <p>{t("backup.noBundle")}</p>
                <button className="ref-btn" onClick={() => void pick()}>
                  <Upload size={13} strokeWidth={1.8} />
                  {t("backup.pick")}
                </button>
              </div>
            )}

            {inspecting && (
              <div className="bk-empty">
                <LoaderCircle size={18} strokeWidth={1.6} className="bk-spin" />
                <p>{t("backup.inspecting")}</p>
              </div>
            )}

            {plan && !inspecting && (
              <motion.div
                className="bk-card"
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                transition={spring}
              >
                <div className="bk-plan-head">
                  <div className="bk-plan-title">
                    <FileArchive size={14} strokeWidth={1.7} />
                    {plan.manifest.machine || t("backup.unknownMachine")}
                  </div>
                  <span className="bk-plan-meta">
                    {t("backup.madeBy", {
                      app: `v${plan.manifest.app}`,
                      when: clock(plan.manifest.exportedAt),
                    })}
                  </span>
                </div>

                <div className="bk-chips">
                  <CountChip value={plan.counts.tags} label={t("backup.countTags")} />
                  <CountChip value={plan.counts.collections} label={t("backup.countCollections")} />
                  <CountChip value={plan.counts.favorites} label={t("backup.countFavorites")} />
                  <CountChip value={plan.counts.ratings} label={t("backup.countRatings")} />
                  <CountChip value={plan.counts.aliases} label={t("backup.countAliases")} />
                  <CountChip value={plan.counts.notes} label={t("backup.countNotes")} />
                  <CountChip value={plan.manifest.files} label={t("backup.countFonts")} />
                </div>

                {plan.unknownTotal > 0 && (
                  <p className="bk-note">
                    <Info size={12} strokeWidth={1.8} />
                    <span>
                      {t("backup.unknownTitle", { count: plan.unknownTotal })}
                      {t("backup.unknownHint")}
                    </span>
                    <span className="bk-note-list">{plan.unknown.slice(0, 6).join(t("backup.listSep"))}</span>
                  </p>
                )}

                {plan.stalePaths > 0 && (
                  <p className="bk-note warn">
                    <ShieldAlert size={12} strokeWidth={1.8} />
                    <span>{t("backup.staleHint", { count: plan.stalePaths })}</span>
                  </p>
                )}

                <div className="bk-modes">
                  <span className="bk-modes-label">{t("backup.modeLabel")}</span>
                  <button
                    className={`bk-mode ${mode === "merge" ? "is-on" : ""}`}
                    onClick={() => setMode("merge")}
                  >
                    <strong>{t("backup.mode.merge")}</strong>
                    <span>{t("backup.mode.mergeHint")}</span>
                  </button>
                  <button
                    className={`bk-mode ${mode === "replace" ? "is-on danger" : ""}`}
                    onClick={() => setMode("replace")}
                  >
                    <strong>{t("backup.mode.replace")}</strong>
                    <span>{t("backup.mode.replaceHint")}</span>
                  </button>
                </div>

                <div className="bk-card-foot">
                  <button className="ref-btn" onClick={() => void pick()}>
                    {t("backup.pickAgain")}
                  </button>
                  <button
                    className={`ref-btn ${mode === "replace" ? "ref-btn-danger" : "primary"}`}
                    onClick={() => void run()}
                    disabled={running}
                  >
                    {running ? (
                      <LoaderCircle size={13} strokeWidth={1.8} className="bk-spin" />
                    ) : (
                      <Check size={13} strokeWidth={2} />
                    )}
                    {running
                      ? t("backup.running")
                      : mode === "replace"
                        ? t("backup.runReplace")
                        : t("backup.run")}
                  </button>
                </div>
              </motion.div>
            )}

            {result && (
              <motion.div
                className="bk-card bk-done"
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                transition={spring}
              >
                <div className="bk-done-head">
                  <Check size={14} strokeWidth={2.2} />
                  {t("backup.doneTitle")}
                </div>
                <p className="bk-done-line">{t("backup.doneSummary", summaryVars(result))}</p>
                {result.fontsWritten + result.fontsSkipped > 0 && (
                  <p className="bk-done-line dim">
                    {t("backup.doneFonts", {
                      written: result.fontsWritten,
                      skipped: result.fontsSkipped,
                    })}
                  </p>
                )}
                {result.usage > 0 && (
                  <p className="bk-done-line dim">{t("backup.doneUsage", { count: result.usage })}</p>
                )}
                {result.settings && (
                  <p className="bk-done-line dim">{t("backup.doneSettings")}</p>
                )}
                {result.stalePaths > 0 && (
                  <p className="bk-done-line dim">
                    {t("backup.doneStale", { count: result.stalePaths })}
                  </p>
                )}
                {result.droppedDirs > 0 && (
                  <p className="bk-done-line dim">
                    {t("backup.doneDirs", { count: result.droppedDirs })}
                  </p>
                )}
                <div className="bk-card-foot">
                  <button
                    className="ref-btn"
                    onClick={() => {
                      setResult(null);
                      void pick();
                    }}
                  >
                    {t("backup.again")}
                  </button>
                </div>
              </motion.div>
            )}
          </section>

          <section className="ref-section">
            <div className="ref-section-head">
              <History size={13} strokeWidth={1.8} />
              {t("audit.title")}
              <span className="bk-head-actions">
                <button className="ref-btn" onClick={() => void exportAudit()} disabled={audit.length === 0}>
                  <Download size={12} strokeWidth={1.8} />
                  {t("audit.export")}
                </button>
                <button
                  className={`ref-btn ${confirmClear ? "ref-btn-danger" : ""}`}
                  onClick={() => void clearAudit()}
                  onBlur={() => setConfirmClear(false)}
                  disabled={audit.length === 0}
                >
                  <Trash2 size={12} strokeWidth={1.8} />
                  {confirmClear ? t("audit.clearConfirm") : t("audit.clear")}
                </button>
              </span>
            </div>
            <p className="us-hint">{t("audit.subtitle")}</p>

            {auditLoading && (
              <div className="bk-empty">
                <LoaderCircle size={18} strokeWidth={1.6} className="bk-spin" />
              </div>
            )}

            {!auditLoading && audit.length === 0 && (
              <div className="bk-empty">
                <History size={20} strokeWidth={1.4} />
                <p>{t("audit.emptyTitle")}</p>
                <p className="ref-empty-sub">{t("audit.emptySub")}</p>
              </div>
            )}

            {!auditLoading && audit.length > 0 && (
              <div className="us-list">
                {audit.map((entry, i) => {
                  const detail = auditDetail(entry, t);
                  return (
                    <motion.div
                      key={entry.id}
                      className="au-row"
                      initial={{ opacity: 0, y: 5 }}
                      animate={{ opacity: 1, y: 0 }}
                      transition={{ ...spring, delay: staggerDelay(i) }}
                    >
                      <span className="au-body">
                        <span className="au-label">{auditLabel(entry, t)}</span>
                        {detail && <span className="au-detail">{detail}</span>}
                      </span>
                      <span className="au-when tabular" title={clock(entry.at)}>
                        {ago(entry.at, t)}
                      </span>
                    </motion.div>
                  );
                })}
              </div>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}
