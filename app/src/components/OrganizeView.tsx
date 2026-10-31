import { open } from "@tauri-apps/plugin-dialog";
import { motion } from "motion/react";
import {
  AlertTriangle,
  Check,
  Copy,
  FolderInput,
  FolderSearch,
  FolderTree,
  HardDriveDownload,
  Link2,
  LoaderCircle,
  RefreshCw,
  ShieldAlert,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { spring, staggerDelay } from "../design/springs";
import { formatBytes } from "../lib/fontLoader";
import { useT } from "../lib/i18n";
import { ipc, type OrganizeItem, type OrganizePlan, type OrganizeResult } from "../lib/ipc";
import { toast } from "../design/primitives/Toast";
import { useFontStore } from "../state/fontStore";

const KEYS = ["vendor", "language", "format", "classification"] as const;
const MODES = ["copy", "move", "link"] as const;
type Key = (typeof KEYS)[number];
type Mode = (typeof MODES)[number];

const MODE_ICON: Record<Mode, React.ReactNode> = {
  copy: <Copy size={13} strokeWidth={1.7} />,
  move: <FolderInput size={13} strokeWidth={1.7} />,
  link: <Link2 size={13} strokeWidth={1.7} />,
};

function baseName(path: string) {
  const parts = path.split(/[\\/]/);
  return parts[parts.length - 1] || path;
}

/** 目标路径去掉目标根目录之后的那一段：`Adobe\思源宋体 (2).ttc`。 */
function relativeTarget(target: string, dest: string) {
  const root = dest.replace(/[\\/]+$/, "");
  if (root && target.toLowerCase().startsWith(root.toLowerCase())) {
    return target.slice(root.length).replace(/^[\\/]+/, "");
  }
  return target;
}

/** 路径是否在目录里。按分隔符判断 —— 纯前缀会把 `E:\字体重新分类` 当成 `E:\字体` 的子目录。 */
function withinDir(target: string, dir: string) {
  const t = target.trim().replace(/[\\/]+$/, "").toLowerCase();
  const d = dir.trim().replace(/[\\/]+$/, "").toLowerCase();
  if (!d) return false;
  return t === d || t.startsWith(`${d}\\`) || t.startsWith(`${d}/`);
}

/** 同一路径的多个字重只算一个文件；`active` 取其中任意一个为真。 */
function itemsFrom(fonts: ReturnType<typeof useFontStore.getState>["fonts"]): OrganizeItem[] {
  const byPath = new Map<string, OrganizeItem>();
  for (const face of fonts) {
    const seen = byPath.get(face.path);
    if (seen) {
      seen.active ||= face.active;
      continue;
    }
    byPath.set(face.path, {
      path: face.path,
      family: face.family,
      foundry: face.foundry,
      scripts: face.scripts ?? [],
      format: face.format,
      classification: face.classification,
      size: face.fileSize,
      source: face.source,
      active: face.active,
    });
  }
  return [...byPath.values()];
}

export function OrganizeView() {
  const t = useT();
  const fonts = useFontStore((s) => s.fonts);
  const settings = useFontStore((s) => s.settings);
  const rescan = useFontStore((s) => s.rescan);

  const [srcMode, setSrcMode] = useState<"library" | "folder">("library");
  const [srcDir, setSrcDir] = useState("");
  const [scanned, setScanned] = useState<OrganizeItem[] | null>(null);
  const [scanBusy, setScanBusy] = useState(false);
  const [scanError, setScanError] = useState<string | null>(null);
  const [key, setKey] = useState<Key>("vendor");
  const [mode, setMode] = useState<Mode>("copy");
  const [dest, setDest] = useState("");
  const [destTouched, setDestTouched] = useState(false);
  const [plan, setPlan] = useState<OrganizePlan | null>(null);
  const [planError, setPlanError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<OrganizeResult | null>(null);
  const [confirmMove, setConfirmMove] = useState(false);
  const [familyRoot, setFamilyRoot] = useState<string>("");
  const [showAll, setShowAll] = useState(false);
  const runId = useRef(0);

  const libraryItems = useMemo(() => itemsFrom(fonts), [fonts]);
  const items = useMemo(
    () => (srcMode === "library" ? libraryItems : (scanned ?? [])),
    [srcMode, libraryItems, scanned],
  );

  // 指定了文件夹就直接解析里面的字体；只读，不建索引，也不进字体库。
  useEffect(() => {
    if (srcMode !== "folder" || !srcDir.trim()) {
      setScanned(null);
      setScanError(null);
      return;
    }
    let alive = true;
    setScanBusy(true);
    const timer = setTimeout(() => {
      ipc
        .organizeScan(srcDir.trim())
        .then((list) => {
          if (!alive) return;
          setScanned(list);
          setScanError(null);
        })
        .catch((e) => {
          if (!alive) return;
          setScanned([]);
          setScanError(String(e));
        })
        .finally(() => {
          if (alive) setScanBusy(false);
        });
    }, 200);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [srcMode, srcDir]);

  // 默认落在库目录下的「归档」子文件夹里 —— 那本来就在扫描范围内，复制完立刻能看见。
  useEffect(() => {
    if (destTouched) return;
    let alive = true;
    void (async () => {
      const base =
        settings.libraryDirEnabled && settings.libraryDir
          ? settings.libraryDir
          : await ipc.defaultLibraryDir();
      if (!alive) return;
      setFamilyRoot(base);
      setDest(`${base}\\归档`);
    })();
    return () => {
      alive = false;
    };
  }, [destTouched, settings.libraryDir, settings.libraryDirEnabled]);

  useEffect(() => {
    if (!dest.trim() || items.length === 0) {
      setPlan(null);
      return;
    }
    const id = ++runId.current;
    setBusy(true);
    const timer = setTimeout(() => {
      ipc
        .organizePlan(items, dest, key, mode)
        .then((next) => {
          if (runId.current !== id) return;
          setPlan(next);
          setPlanError(null);
        })
        .catch((e) => {
          if (runId.current !== id) return;
          setPlan(null);
          setPlanError(String(e));
        })
        .finally(() => {
          if (runId.current === id) setBusy(false);
        });
    }, 260);
    return () => clearTimeout(timer);
  }, [items, dest, key, mode]);

  const inLibrary = useMemo(() => {
    const dirs = [familyRoot, settings.libraryDir ?? "", ...settings.extraDirs].filter(Boolean);
    return dirs.some((d) => withinDir(dest, d));
  }, [dest, familyRoot, settings.libraryDir, settings.extraDirs]);

  const pickSource = async () => {
    const dir = await open({ directory: true, title: t("org.pickSource") });
    if (dir) {
      setSrcDir(dir);
      setSrcMode("folder");
    }
  };

  const pickDest = async () => {
    const dir = await open({ directory: true, title: t("org.pickDest") });
    if (dir) {
      setDest(dir);
      setDestTouched(true);
    }
  };

  const run = async () => {
    if (!plan || plan.ready === 0) return;
    if (mode === "move" && !confirmMove) {
      setConfirmMove(true);
      return;
    }
    setConfirmMove(false);
    setBusy(true);
    setResult(null);
    try {
      const out = await ipc.organizeApply(items, dest, key, mode);
      setResult(out);
      if (out.done > 0) {
        toast.success(
          t("org.toastDone", { count: out.done, size: formatBytes(out.bytes) }),
          mode === "move" ? t("org.toastMoved") : t("org.toastCopied"),
          "install",
        );
        await rescan();
      }
      if (out.failed > 0) toast.error(t("org.toastFailed", { count: out.failed }), "");
    } catch (e) {
      toast.error(t("org.toastError"), String(e));
    } finally {
      setBusy(false);
    }
  };

  const skippedByReason = useMemo(() => {
    const map = new Map<string, number>();
    for (const entry of plan?.entries ?? []) {
      if (entry.action !== "skip") continue;
      map.set(entry.reason, (map.get(entry.reason) ?? 0) + 1);
    }
    return [...map.entries()].sort((a, b) => b[1] - a[1]);
  }, [plan]);

  const readyEntries = useMemo(
    () => (plan?.entries ?? []).filter((e) => e.action !== "skip"),
    [plan],
  );
  const shown = showAll ? readyEntries : readyEntries.slice(0, 200);
  const failures = result?.outcomes.filter((o) => o.status === "failed") ?? [];

  return (
    <div className="ref-view">
      <div className="ref-inner">
        <header className="ref-head">
          <div className="ref-head-text">
            <h1 className="ref-title">{t("org.title")}</h1>
            <p className="ref-sub">{t("org.subtitle")}</p>
          </div>
          <div className="ref-head-actions">
            <button
              className="ref-btn"
              disabled={busy}
              onClick={() => {
                setResult(null);
                setDestTouched(false);
                setSrcMode("library");
                setSrcDir("");
                setScanned(null);
                setScanError(null);
              }}
            >
              <RefreshCw size={13} strokeWidth={1.7} className={busy ? "fd-spin" : ""} />
              {t("org.reset")}
            </button>
          </div>
        </header>

        <div className="ref-body">
          <section className="og-card">
            <div className="og-step">
              <span className="og-step-index">1</span>
              {t("org.sourceLabel")}
            </div>
            <div className="og-chips">
              <button
                className={`og-chip ${srcMode === "library" ? "og-chip-on" : ""}`}
                onClick={() => setSrcMode("library")}
              >
                {t("org.source.library")}
              </button>
              <button
                className={`og-chip ${srcMode === "folder" ? "og-chip-on" : ""}`}
                onClick={() => {
                  if (srcDir.trim()) setSrcMode("folder");
                  else void pickSource();
                }}
              >
                {t("org.source.folder")}
              </button>
            </div>
            {srcMode === "library" ? (
              <p className="us-hint">
                {t("org.source.libraryHint", { count: libraryItems.length })}
              </p>
            ) : (
              <>
                <div className="og-dest">
                  <input
                    className="og-input fd-mono"
                    value={srcDir}
                    spellCheck={false}
                    placeholder={t("org.sourceEmpty")}
                    onChange={(e) => setSrcDir(e.target.value)}
                    aria-label={t("org.source.folder")}
                  />
                  <button className="ref-btn" onClick={() => void pickSource()}>
                    <FolderSearch size={13} strokeWidth={1.7} />
                    {t("org.pickSource")}
                  </button>
                </div>
                <p className="us-hint">
                  {scanBusy
                    ? t("org.scanning")
                    : scanError
                      ? t("org.scanFailed")
                      : !srcDir.trim()
                        ? t("org.sourceEmpty")
                        : items.length === 0
                          ? t("org.sourceNoFonts")
                          : t("org.source.folderHint")}
                </p>
              </>
            )}

            <div className="og-step">
              <span className="og-step-index">2</span>
              {t("org.ruleLabel")}
            </div>
            <div className="og-chips">
              {KEYS.map((k) => (
                <button
                  key={k}
                  className={`og-chip ${key === k ? "og-chip-on" : ""}`}
                  onClick={() => setKey(k)}
                >
                  {t(`org.key.${k}`)}
                </button>
              ))}
            </div>
            <p className="us-hint">{t(`org.key.${key}Hint`)}</p>

            <div className="og-step">
              <span className="og-step-index">3</span>
              {t("org.destLabel")}
            </div>
            <div className="og-dest">
              <input
                className="og-input fd-mono"
                value={dest}
                spellCheck={false}
                onChange={(e) => {
                  setDest(e.target.value);
                  setDestTouched(true);
                }}
                aria-label={t("org.destLabel")}
              />
              <button className="ref-btn" onClick={() => void pickDest()}>
                <FolderTree size={13} strokeWidth={1.7} />
                {t("org.pickDest")}
              </button>
            </div>

            <div className="og-step">
              <span className="og-step-index">4</span>
              {t("org.modeLabel")}
            </div>
            <div className="og-chips">
              {MODES.map((m) => (
                <button
                  key={m}
                  className={`og-chip ${mode === m ? "og-chip-on" : ""} ${m === "move" ? "og-chip-warn" : ""}`}
                  onClick={() => {
                    setMode(m);
                    setConfirmMove(false);
                  }}
                >
                  {MODE_ICON[m]}
                  {t(`org.mode.${m}`)}
                </button>
              ))}
            </div>
            <p className="us-hint">{t(`org.mode.${mode}Hint`)}</p>
          </section>

          {planError && (
            <div className="og-alert">
              <AlertTriangle size={14} strokeWidth={1.8} />
              {planError}
            </div>
          )}

          {mode === "move" && !inLibrary && (
            <div className="og-alert">
              <ShieldAlert size={14} strokeWidth={1.8} />
              {t("org.outsideLibrary")}
            </div>
          )}

          {plan && (
            <>
              <div className="ref-stats">
                <div className="ref-stat primary">
                  <span className="ref-stat-value tabular">{plan.ready}</span>
                  <span className="ref-stat-label">{t("org.statReady")}</span>
                </div>
                <div className="ref-stat">
                  <span className="ref-stat-value tabular">{plan.folders.length}</span>
                  <span className="ref-stat-label">{t("org.statFolders")}</span>
                </div>
                <div className="ref-stat">
                  <span className="ref-stat-value tabular">{formatBytes(plan.bytes)}</span>
                  <span className="ref-stat-label">{t("org.statBytes")}</span>
                </div>
                <div className="ref-stat">
                  <span className="ref-stat-value tabular">{plan.skipped}</span>
                  <span className="ref-stat-label">{t("org.statSkipped")}</span>
                </div>
                {plan.collisions > 0 && (
                  <div className="ref-stat dim">
                    <span className="ref-stat-value tabular">{plan.collisions}</span>
                    <span className="ref-stat-label">{t("org.statCollisions")}</span>
                  </div>
                )}
              </div>

              <div className="og-folders">
                {plan.folders.map((folder) => (
                  <div className="og-folder" key={folder.name}>
                    <FolderTree size={12} strokeWidth={1.7} />
                    <span className="og-folder-name">{folder.name}</span>
                    <span className="og-folder-count tabular">{folder.count}</span>
                  </div>
                ))}
              </div>

              {plan.ready === 0 ? (
                <p className="ref-none">{t("org.nothingToDo")}</p>
              ) : (
                <section className="ref-section">
                  <div className="ref-section-head">
                    <Check size={13} strokeWidth={2} />
                    {t("org.previewTitle", { count: plan.ready })}
                  </div>
                  <div className="og-list">
                    {shown.map((entry, i) => (
                      <motion.div
                        className="og-row"
                        key={entry.path}
                        initial={{ opacity: 0, y: 5 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ ...spring, delay: staggerDelay(i) }}
                      >
                        <span className="og-row-folder">{entry.folder}</span>
                        <span className="og-row-file" title={entry.path}>
                          {baseName(entry.path)}
                        </span>
                        <span className="og-row-arrow">→</span>
                        <span className="og-row-target fd-mono" title={entry.target}>
                          {relativeTarget(entry.target, plan.dest)}
                        </span>
                      </motion.div>
                    ))}
                  </div>
                  {readyEntries.length > shown.length && (
                    <button className="us-more" onClick={() => setShowAll(true)}>
                      {t("org.showAll", { count: readyEntries.length })}
                    </button>
                  )}
                </section>
              )}

              {skippedByReason.length > 0 && (
                <section className="ref-section">
                  <div className="ref-section-head">
                    <AlertTriangle size={13} strokeWidth={1.8} />
                    {t("org.skipTitle")}
                  </div>
                  <div className="og-skips">
                    {skippedByReason.map(([reason, count]) => (
                      <div className="og-skip" key={reason}>
                        <span>{reason}</span>
                        <span className="tabular">{count}</span>
                      </div>
                    ))}
                  </div>
                </section>
              )}

              <div className="og-run">
                {mode === "move" && (
                  <p className="og-warn">{t("org.moveWarn", { count: plan.ready })}</p>
                )}
                <button
                  className={`ref-btn primary og-run-btn ${confirmMove ? "og-run-confirm" : ""}`}
                  disabled={busy || plan.ready === 0}
                  onClick={() => void run()}
                >
                  {busy ? (
                    <LoaderCircle size={13} strokeWidth={1.7} className="fd-spin" />
                  ) : mode === "move" ? (
                    <FolderInput size={13} strokeWidth={1.7} />
                  ) : (
                    <HardDriveDownload size={13} strokeWidth={1.7} />
                  )}
                  {confirmMove
                    ? t("org.confirmMove", { count: plan.ready })
                    : mode === "move"
                      ? t("org.runMove", { count: plan.ready })
                      : t("org.run", { count: plan.ready })}
                </button>
              </div>
            </>
          )}

          {result && (
            <section className="ref-section">
              <div className="ref-section-head">
                <Check size={13} strokeWidth={2} />
                {t("org.resultTitle")}
              </div>
              <p className="us-hint">
                {t("org.resultSummary", {
                  done: result.done,
                  failed: result.failed,
                  size: formatBytes(result.bytes),
                })}
              </p>
              <div className="og-dest-result fd-mono">{result.dest}</div>
              {failures.length > 0 && (
                <div className="og-skips">
                  {failures.slice(0, 20).map((f) => (
                    <div className="og-skip" key={f.path}>
                      <span>
                        {f.family} · {f.message}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </section>
          )}
        </div>
      </div>
    </div>
  );
}
