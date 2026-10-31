import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { AnimatePresence, motion } from "motion/react";
import {
  AlertTriangle,
  Check,
  FolderOpen,
  FolderSearch,
  LoaderCircle,
  RefreshCw,
} from "lucide-react";
import { useMemo } from "react";
import { spring, staggerDelay } from "../design/springs";
import { formatBytes } from "../lib/fontLoader";
import { useT } from "../lib/i18n";
import type { RefFile } from "../lib/ipc";
import { useFontStore } from "../state/fontStore";

function FileRow({ file }: { file: RefFile }) {
  const t = useT();
  return (
    <div className="ref-row">
      <div className="ref-row-main">
        <div className="ref-row-head">
          <span className="ref-file tabular" title={file.path}>
            {file.fileName}
          </span>
          <span className="ref-tag">{file.kind.toUpperCase()}</span>
          <span className="ref-meta tabular">{formatBytes(file.size)}</span>
          <button
            className="ref-reveal"
            title={t("ref.reveal")}
            aria-label={t("ref.reveal")}
            onClick={() => void revealItemInDir(file.path).catch(() => {})}
          >
            <FolderOpen size={12} strokeWidth={1.7} />
          </button>
        </div>
        <div className="ref-path fd-mono" title={file.path}>
          {file.path}
        </div>
        <div className="ref-chips">
          {file.missing.map((name) => (
            <span className="ref-chip missing" key={`m-${name}`}>
              {name}
            </span>
          ))}
          {file.installed.map((name) => (
            <span className="ref-chip" key={`i-${name}`}>
              {name}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}

export function ReferencesView() {
  const t = useT();
  const report = useFontStore((s) => s.refReport);
  const loading = useFontStore((s) => s.refLoading);
  const root = useFontStore((s) => s.refRoot);
  const scan = useFontStore((s) => s.scanReferences);

  const pick = async () => {
    const dir = await open({ directory: true, title: t("ref.pickTitle") });
    if (dir) void scan(dir);
  };

  const missing = useMemo(() => report?.fonts.filter((f) => !f.installed) ?? [], [report]);
  const hit = useMemo(() => report?.fonts.filter((f) => f.installed) ?? [], [report]);

  return (
    <div className="ref-view">
      <div className="ref-inner">
        <header className="ref-head">
          <div className="ref-head-text">
            <h1 className="ref-title">{t("ref.title")}</h1>
            <p className="ref-sub">{t("ref.subtitle")}</p>
          </div>
          <div className="ref-head-actions">
            {report && (
              <button className="ref-btn" disabled={loading} onClick={() => root && void scan(root)}>
                <RefreshCw size={13} strokeWidth={1.7} className={loading ? "fd-spin" : ""} />
                {t("ref.rescan")}
              </button>
            )}
            <button className="ref-btn primary" disabled={loading} onClick={() => void pick()}>
              <FolderSearch size={13} strokeWidth={1.7} />
              {t("ref.pick")}
            </button>
          </div>
        </header>

        {loading && (
          <div className="ref-loading">
            <LoaderCircle size={15} strokeWidth={1.7} className="fd-spin" />
            {t("ref.scanning")}
          </div>
        )}

        {!loading && !report && (
          <div className="ref-empty">
            <FolderSearch size={22} strokeWidth={1.4} />
            <p>{t("ref.emptyHint")}</p>
            <p className="ref-empty-sub">{t("ref.emptySub")}</p>
          </div>
        )}

        {!loading && report && (
          <AnimatePresence initial={false}>
            <motion.div
              className="ref-body"
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              transition={spring}
            >
              <div className="ref-stats">
                <div className="ref-stat">
                  <span className="ref-stat-value tabular">{report.scanned.toLocaleString()}</span>
                  <span className="ref-stat-label">{t("ref.statScanned")}</span>
                </div>
                <div className="ref-stat primary">
                  <span className="ref-stat-value tabular">{report.missingCount}</span>
                  <span className="ref-stat-label">{t("ref.statMissing")}</span>
                </div>
                <div className="ref-stat">
                  <span className="ref-stat-value tabular">{report.installedCount}</span>
                  <span className="ref-stat-label">{t("ref.statInstalled")}</span>
                </div>
                <div className="ref-stat">
                  <span className="ref-stat-value tabular">
                    {report.files.length.toLocaleString()}
                  </span>
                  <span className="ref-stat-label">{t("ref.statFiles")}</span>
                </div>
                <div className="ref-stat dim">
                  <span className="ref-stat-value tabular">{(report.elapsedMs / 1000).toFixed(1)}s</span>
                  <span className="ref-stat-label">{t("ref.statTime")}</span>
                </div>
              </div>

              <div className="ref-root fd-mono" title={report.root}>
                {report.root}
              </div>

              {missing.length > 0 && (
                <section className="ref-section">
                  <div className="ref-section-head">
                    <AlertTriangle size={13} strokeWidth={1.8} />
                    {t("ref.missingSection", { count: missing.length })}
                  </div>
                  <div className="ref-font-list">
                    {missing.map((f) => (
                      <div className="ref-font" key={f.name}>
                        <span className="ref-font-name">{f.name}</span>
                        <span className="ref-font-count tabular">
                          {t("ref.fileCount", { count: f.files })}
                        </span>
                      </div>
                    ))}
                  </div>
                </section>
              )}

              {hit.length > 0 && (
                <section className="ref-section">
                  <div className="ref-section-head">
                    <Check size={13} strokeWidth={2} />
                    {t("ref.installedSection", { count: hit.length })}
                  </div>
                  <div className="ref-font-list">
                    {hit.map((f) => (
                      <div className="ref-font" key={f.name}>
                        <span className="ref-font-name">{f.name}</span>
                        <span className="ref-font-count tabular">
                          {t("ref.fileCount", { count: f.files })}
                        </span>
                      </div>
                    ))}
                  </div>
                </section>
              )}

              <section className="ref-section">
                <div className="ref-section-head">
                  <FolderOpen size={13} strokeWidth={1.8} />
                  {t("ref.filesSection", { count: report.files.length })}
                </div>
                {report.files.length === 0 ? (
                  <p className="ref-none">{t("ref.noFiles")}</p>
                ) : (
                  <div className="ref-files">
                    {report.files.map((f, i) => (
                      <motion.div
                        key={f.path}
                        initial={{ opacity: 0, y: 6 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ ...spring, delay: staggerDelay(i) }}
                      >
                        <FileRow file={f} />
                      </motion.div>
                    ))}
                  </div>
                )}
              </section>
            </motion.div>
          </AnimatePresence>
        )}
      </div>
    </div>
  );
}
