import { open, save } from "@tauri-apps/plugin-dialog";
import { AnimatePresence, motion } from "motion/react";
import {
  Boxes,
  Download,
  FileDown,
  Globe2,
  LoaderCircle,
  PencilLine,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { springSnappy } from "../design/springs";
import { toast } from "../design/primitives/Toast";
import { formatBytes } from "../lib/fontLoader";
import {
  ipc,
  type SubsetCharset,
  type SubsetFormat,
} from "../lib/ipc";
import { useT } from "../lib/i18n";

interface Props {
  open: boolean;
  onClose: () => void;
  path: string;
  faceIndex: number;
  family: string;
  style: string;
  format: string;
  facesInFile: number;
}

const CHARSETS = [
  { key: "latin", labelKey: "export.charset.latin" },
  { key: "zh", labelKey: "export.charset.zh" },
  { key: "zhFull", labelKey: "export.charset.zhFull" },
  { key: "text", labelKey: "export.charset.text" },
  { key: "all", labelKey: "export.charset.all" },
] as const;

const FORMATS: SubsetFormat[] = ["woff2", "woff", "ttf"];

/** 导出字体：单个字重、集合拆包、改内部命名、网页字体子集化，四条路都在这一个面板里。 */
export function ExportDialog({
  open: isOpen,
  onClose,
  path,
  faceIndex,
  family,
  style,
  format,
  facesInFile,
}: Props) {
  const t = useT();
  const [charset, setCharset] = useState<SubsetCharset>("zh");
  const [text, setText] = useState("");
  const [outFormat, setOutFormat] = useState<SubsetFormat>("woff2");
  const [renaming, setRenaming] = useState(false);
  const [newFamily, setNewFamily] = useState(family);
  const [newStyle, setNewStyle] = useState(style);
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    setNewFamily(family);
    setNewStyle(style);
    setRenaming(false);
    setBusy(null);
  }, [isOpen, family, style]);

  useEffect(() => {
    if (!isOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [isOpen, onClose]);

  const stem = `${family}${style.toLowerCase() === "regular" ? "" : ` ${style}`}`;
  // format 可能是 woff/unknown，导出的是 sfnt 文件，只认 ttf/otf。
  const ext = format === "otf" ? "otf" : "ttf";

  const run = async (id: string, task: () => Promise<void>) => {
    setBusy(id);
    try {
      await task();
      onClose();
    } catch (e) {
      toast.error(t("toast.exportFailed"), String(e));
    } finally {
      setBusy(null);
    }
  };

  const exportSingle = () =>
    run("single", async () => {
      const dest = await save({
        defaultPath: `${stem}.${ext}`,
        title: t("export.titleSingle"),
      });
      if (!dest) return;
      const result = await ipc.exportFontFace(path, faceIndex, dest);
      toast.success(
        t("export.doneSingle"),
        t("export.doneDetail", {
          size: formatBytes(result.byteSize),
          file: result.path,
        }),
      );
    });

  const exportCollection = () =>
    run("collection", async () => {
      const dir = await open({ directory: true, title: t("export.titleCollection") });
      if (!dir) return;
      const results = await ipc.exportFontCollection(path, dir);
      toast.success(
        t("export.doneCollection", { count: results.length }),
        dir,
      );
    });

  const exportRenamed = () =>
    run("rename", async () => {
      if (!newFamily.trim()) {
        toast.error(t("export.nameRequired"), newFamily);
        return;
      }
      const dest = await save({
        defaultPath: `${newFamily}${newStyle.toLowerCase() === "regular" ? "" : ` ${newStyle}`}.${ext}`,
        title: t("export.titleRename"),
      });
      if (!dest) return;
      const result = await ipc.exportFontFaceRenamed(
        path,
        faceIndex,
        dest,
        newFamily.trim(),
        newStyle.trim() || "Regular",
      );
      toast.success(t("export.doneRename"), result.path);
    });

  const exportSubset = () =>
    run("subset", async () => {
      if (charset === "text" && text.trim().length === 0) {
        toast.error(t("export.textRequired"), "");
        return;
      }
      const dest = await save({
        defaultPath: `${stem} (subset).${outFormat}`,
        title: t("export.titleSubset"),
        filters: [{ name: outFormat.toUpperCase(), extensions: [outFormat] }],
      });
      if (!dest) return;
      const result = await ipc.exportFontSubset(
        path,
        faceIndex,
        dest,
        charset,
        text,
        outFormat,
      );
      toast.success(
        t("export.doneSubset"),
        t("export.doneDetail", {
          size: formatBytes(result.byteSize),
          file: result.path,
        }),
      );
    });

  const spin = (id: string) =>
    busy === id ? <LoaderCircle size={13} strokeWidth={1.8} className="fd-spin" /> : null;

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          className="exp-backdrop"
          onClick={onClose}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.14 }}
        >
          <motion.div
            className="exp-panel"
            role="dialog"
            aria-modal="true"
            aria-label={t("export.title")}
            onClick={(e) => e.stopPropagation()}
            initial={{ opacity: 0, scale: 0.97, y: 10 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.98, y: 6, transition: { duration: 0.12 } }}
            transition={springSnappy}
          >
            <header className="exp-head">
              <div className="exp-head-text">
                <span className="exp-title">{t("export.title")}</span>
                <span className="exp-sub" title={path}>
                  {stem} · {format.toUpperCase()}
                  {facesInFile > 1 ? ` · ${t("export.inCollection", { count: facesInFile })}` : ""}
                </span>
              </div>
              <button className="fd-icon-btn" onClick={onClose} aria-label={t("export.close")}>
                <X size={14} strokeWidth={1.7} />
              </button>
            </header>

            <div className="exp-body">
              <section className="exp-block">
                <div className="exp-block-head">
                  <FileDown size={13} strokeWidth={1.7} />
                  {t("export.sectionFile")}
                </div>
                <p className="exp-note">{t("export.fileNote")}</p>
                <div className="exp-actions">
                  <button className="exp-btn primary" disabled={busy !== null} onClick={exportSingle}>
                    {spin("single")}
                    {t("export.actionSingle")}
                  </button>
                  {facesInFile > 1 && (
                    <button
                      className="exp-btn"
                      disabled={busy !== null}
                      onClick={exportCollection}
                    >
                      {spin("collection") ?? <Boxes size={13} strokeWidth={1.7} />}
                      {t("export.actionCollection", { count: facesInFile })}
                    </button>
                  )}
                  <button
                    className={`exp-btn ${renaming ? "on" : ""}`}
                    disabled={busy !== null}
                    onClick={() => setRenaming((v) => !v)}
                  >
                    <PencilLine size={13} strokeWidth={1.7} />
                    {t("export.actionRename")}
                  </button>
                </div>

                <AnimatePresence initial={false}>
                  {renaming && (
                    <motion.div
                      className="exp-rename"
                      initial={{ height: 0, opacity: 0 }}
                      animate={{ height: "auto", opacity: 1 }}
                      exit={{ height: 0, opacity: 0 }}
                      transition={{ duration: 0.16 }}
                    >
                      <label className="exp-field">
                        <span>{t("export.familyName")}</span>
                        <input
                          value={newFamily}
                          onChange={(e) => setNewFamily(e.target.value)}
                          spellCheck={false}
                        />
                      </label>
                      <label className="exp-field">
                        <span>{t("export.styleName")}</span>
                        <input
                          value={newStyle}
                          onChange={(e) => setNewStyle(e.target.value)}
                          spellCheck={false}
                        />
                      </label>
                      <button className="exp-btn primary" disabled={busy !== null} onClick={exportRenamed}>
                        {spin("rename")}
                        {t("export.actionRenameGo")}
                      </button>
                      <p className="exp-note">{t("export.renameNote")}</p>
                    </motion.div>
                  )}
                </AnimatePresence>
              </section>

              <section className="exp-block">
                <div className="exp-block-head">
                  <Globe2 size={13} strokeWidth={1.7} />
                  {t("export.sectionSubset")}
                </div>
                <p className="exp-note">{t("export.subsetNote")}</p>

                <div className="exp-chips">
                  {CHARSETS.map((c) => (
                    <button
                      key={c.key}
                      className={`exp-chip ${charset === c.key ? "on" : ""}`}
                      onClick={() => setCharset(c.key)}
                    >
                      {t(c.labelKey)}
                    </button>
                  ))}
                </div>

                {charset === "text" && (
                  <textarea
                    className="exp-textarea"
                    value={text}
                    onChange={(e) => setText(e.target.value)}
                    placeholder={t("export.textPlaceholder")}
                    rows={3}
                    spellCheck={false}
                  />
                )}

                <div className="exp-chips">
                  {FORMATS.map((f) => (
                    <button
                      key={f}
                      className={`exp-chip ${outFormat === f ? "on" : ""}`}
                      onClick={() => setOutFormat(f)}
                    >
                      {f.toUpperCase()}
                    </button>
                  ))}
                </div>

                <div className="exp-actions">
                  <button className="exp-btn primary" disabled={busy !== null} onClick={exportSubset}>
                    {spin("subset") ?? <Download size={13} strokeWidth={1.7} />}
                    {t("export.actionSubset", { format: outFormat.toUpperCase() })}
                  </button>
                </div>
              </section>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
