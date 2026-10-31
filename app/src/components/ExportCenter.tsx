import { save } from "@tauri-apps/plugin-dialog";
import { motion } from "motion/react";
import {
  ClipboardCopy,
  FileJson,
  LoaderCircle,
  Package,
  Table2,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { springSnappy } from "../design/springs";
import { toast } from "../design/primitives/Toast";
import { formatBytes } from "../lib/fontLoader";
import { activeLocale, useT } from "../lib/i18n";
import { ipc } from "../lib/ipc";
import { rowsFor, safeName } from "../lib/exporter";
import { useFontStore } from "../state/fontStore";

/**
 * 导出面板（P7）：清单 CSV/JSON、项目字体包 zip、@font-face 片段。
 * 作用范围由 `exportScope` 决定 —— 右键选中项、详情页、集合都能打开同一个面板。
 */
export function ExportCenter() {
  const t = useT();
  const scope = useFontStore((s) => s.exportScope);
  const closeExport = useFontStore((s) => s.closeExport);
  // 这几个只用于「重新算行」：标签、备注、集合改过之后清单要跟着变。
  const fonts = useFontStore((s) => s.fonts);
  const tags = useFontStore((s) => s.tags);
  const collections = useFontStore((s) => s.collections);
  const favorites = useFontStore((s) => s.favorites);
  const notes = useFontStore((s) => s.notes);

  const open = scope !== null;

  const rows = useMemo(
    () => (scope ? rowsFor(scope.names) : []),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [scope, fonts, tags, collections, favorites, notes],
  );

  const [name, setName] = useState("");
  const [withFiles, setWithFiles] = useState(true);
  const [web, setWeb] = useState(false);
  const [withCss, setWithCss] = useState(true);
  const [css, setCss] = useState("");
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    if (!scope) return;
    setName(safeName(scope.label));
    setBusy(null);
    setWeb(false);
  }, [scope]);

  useEffect(() => {
    if (!open) return;
    let alive = true;
    void ipc
      .fontFaceCss(rows, web)
      .then((text) => alive && setCss(text))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [open, rows, web]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !busy) {
        e.stopPropagation();
        closeExport();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [open, closeExport, busy]);

  if (!scope) return null;

  const stem = safeName(scope.label);

  const exportCatalog = async (format: "csv" | "json") => {
    if (rows.length === 0) return;
    const dest = await save({
      defaultPath: `${stem}-字体清单.${format}`,
      title: t("export2.catalogTitle"),
      filters: [{ name: format.toUpperCase(), extensions: [format] }],
    });
    if (!dest) return;
    setBusy("catalog");
    try {
      const out = await ipc.exportCatalog(dest, format, rows, activeLocale().startsWith("zh"));
      toast.success(
        t("export2.catalogDone", { count: out.rows, size: formatBytes(out.bytes) }),
        dest,
        "install",
      );
      closeExport();
    } catch (e) {
      toast.error(t("toast.exportFailed"), String(e));
    } finally {
      setBusy(null);
    }
  };

  const buildPackage = async () => {
    if (rows.length === 0) return;
    const dest = await save({
      defaultPath: `${stem}.zip`,
      title: t("export2.packageTitle"),
      filters: [{ name: "ZIP", extensions: ["zip"] }],
    });
    if (!dest) return;
    setBusy("package");
    try {
      const out = await ipc.exportFontPackage(
        dest,
        name.trim() || stem,
        rows,
        withFiles,
        web,
        withCss,
      );
      toast.success(
        t("export2.packageDone", { count: out.fonts, size: formatBytes(out.bytes) }),
        out.errors.length > 0 ? out.errors[0] : dest,
      );
      closeExport();
    } catch (e) {
      toast.error(t("toast.exportFailed"), String(e));
    } finally {
      setBusy(null);
    }
  };

  const copyCss = async () => {
    try {
      await navigator.clipboard.writeText(css);
      toast.success(t("toast.copied", { what: t("copy.fontFaceCss") }), undefined, "tick");
    } catch (e) {
      toast.error(t("toast.couldntCopy"), String(e));
    }
  };

  const spin = (id: string) =>
    busy === id ? <LoaderCircle size={13} strokeWidth={1.8} className="fd-spin" /> : null;

  return (
    <motion.div
      className="exp-backdrop"
      onClick={() => !busy && closeExport()}
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      transition={{ duration: 0.14 }}
    >
      <motion.div
        className="exp-panel exp-panel-wide"
        role="dialog"
        aria-modal="true"
        aria-label={t("export2.title")}
        onClick={(e) => e.stopPropagation()}
        initial={{ opacity: 0, scale: 0.97, y: 10 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        transition={springSnappy}
      >
          <header className="exp-head">
            <div className="exp-head-text">
              <span className="exp-title">{t("export2.title")}</span>
              <span className="exp-sub">
                {scope.label} · {t("export2.scopeFaces", { faces: rows.length })}
              </span>
            </div>
            <button
              className="fd-icon-btn"
              onClick={closeExport}
              aria-label={t("export2.close")}
              disabled={busy !== null}
            >
              <X size={14} strokeWidth={1.7} />
            </button>
          </header>

          <div className="exp-body">
            <section className="exp-block">
              <div className="exp-block-head">
                <Table2 size={13} strokeWidth={1.7} />
                {t("export2.sectionCatalog")}
              </div>
              <p className="exp-note">{t("export2.catalogNote")}</p>
              <div className="exp-actions">
                <button
                  className="exp-btn primary"
                  disabled={busy !== null || rows.length === 0}
                  onClick={() => void exportCatalog("csv")}
                >
                  {spin("catalog")}
                  <Table2 size={13} strokeWidth={1.7} />
                  {t("export2.csv")}
                </button>
                <button
                  className="exp-btn"
                  disabled={busy !== null || rows.length === 0}
                  onClick={() => void exportCatalog("json")}
                >
                  <FileJson size={13} strokeWidth={1.7} />
                  {t("export2.json")}
                </button>
              </div>
            </section>

            <section className="exp-block">
              <div className="exp-block-head">
                <Package size={13} strokeWidth={1.7} />
                {t("export2.sectionPackage")}
              </div>
              <p className="exp-note">{t("export2.packageNote")}</p>

              <label className="exp-field">
                <span>{t("export2.packageName")}</span>
                <input
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  spellCheck={false}
                  placeholder={stem}
                />
              </label>

              <div className="exp-chips">
                <button
                  className={`exp-chip ${withFiles ? "on" : ""}`}
                  onClick={() => setWithFiles((v) => !v)}
                >
                  {t("export2.withFiles")}
                </button>
                <button
                  className={`exp-chip ${web ? "on" : ""}`}
                  onClick={() => setWeb((v) => !v)}
                >
                  {t("export2.withWeb")}
                </button>
                <button
                  className={`exp-chip ${withCss ? "on" : ""}`}
                  onClick={() => setWithCss((v) => !v)}
                >
                  {t("export2.withCss")}
                </button>
              </div>
              <p className="exp-note exp-note-dim">{t("export2.webHint")}</p>

              <div className="exp-actions">
                <button
                  className="exp-btn primary"
                  disabled={busy !== null || rows.length === 0}
                  onClick={() => void buildPackage()}
                >
                  {spin("package") ?? <Package size={13} strokeWidth={1.7} />}
                  {t("export2.buildPackage")}
                </button>
              </div>
            </section>

            <section className="exp-block">
              <div className="exp-block-head">
                <ClipboardCopy size={13} strokeWidth={1.7} />
                {t("export2.sectionCss")}
              </div>
              <p className="exp-note">{t("export2.cssNote")}</p>
              <pre className="exp-css" aria-label={t("export2.sectionCss")}>
                {css}
              </pre>
              <div className="exp-actions">
                <button className="exp-btn" onClick={() => void copyCss()}>
                  <ClipboardCopy size={13} strokeWidth={1.7} />
                  {t("export2.copyCss")}
                </button>
              </div>
            </section>
          </div>
        </motion.div>
      </motion.div>
  );
}
