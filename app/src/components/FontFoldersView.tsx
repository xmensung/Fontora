import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { motion } from "motion/react";
import {
  CheckCircle2,
  CircleDashed,
  FolderCog,
  FolderOpen,
  HardDrive,
  LoaderCircle,
  Plus,
  RefreshCw,
  Trash2,
} from "lucide-react";
import { type ReactNode, useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "../design/primitives/Toast";
import { PillToggle } from "../design/primitives/PillToggle";
import { spring } from "../design/springs";
import { useT } from "../lib/i18n";
import { ipc } from "../lib/ipc";
import { familiesFor, useFontStore, type Family } from "../state/fontStore";

/** Windows 路径比较：不分大小写、斜杠归一、去掉尾部分隔符。 */
function norm(p: string): string {
  return p.toLowerCase().replace(/\//g, "\\").replace(/\\+$/, "");
}

/** 该文件是否位于 dir 之内（含 dir 本身）。 */
function under(path: string, dir: string): boolean {
  const p = norm(path);
  const d = norm(dir);
  return p === d || p.startsWith(`${d}\\`);
}

interface DirStats {
  faces: number;
  families: number;
  missing: number;
}

function countsFor(families: Family[]): DirStats {
  return {
    faces: families.reduce((n, f) => n + f.faces.length, 0),
    families: families.length,
    missing: families.filter((f) => !f.active).length,
  };
}

function DirRow({
  icon,
  path,
  stats,
  tag,
  actions,
  readonly,
  index,
}: {
  icon: ReactNode;
  path: string;
  stats: DirStats;
  tag?: string;
  actions?: ReactNode;
  readonly?: boolean;
  index: number;
}) {
  const t = useT();
  return (
    <motion.div
      className={`fdir-row ${readonly ? "is-readonly" : ""}`}
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ ...spring, delay: Math.min(index, 8) * 0.02 }}
    >
      <span className="fdir-icon">{icon}</span>
      <div className="fdir-text">
        <div className="fdir-pathline">
          <span className="fdir-path detail-mono">{path}</span>
          {tag && <span className="fdir-tag">{tag}</span>}
        </div>
        <div className="fdir-meta">
          <span className="tabular">{t("folders.metaFaces", { count: stats.faces })}</span>
          <span className="fdir-dot">·</span>
          <span className="tabular">{t("folders.metaFamilies", { count: stats.families })}</span>
          {stats.missing > 0 && (
            <>
              <span className="fdir-dot">·</span>
              <span className="fdir-missing tabular">
                {t("folders.metaMissing", { count: stats.missing })}
              </span>
            </>
          )}
        </div>
      </div>
      {actions && <div className="fdir-actions">{actions}</div>}
    </motion.div>
  );
}

export function FontFoldersView() {
  const t = useT();
  const fonts = useFontStore((s) => s.fonts);
  const tags = useFontStore((s) => s.tags);
  const settings = useFontStore((s) => s.settings);
  const phase = useFontStore((s) => s.phase);
  const updateSettings = useFontStore((s) => s.updateSettings);
  const rescan = useFontStore((s) => s.rescan);

  const [managedDir, setManagedDir] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void ipc
      .defaultLibraryDir()
      .then(setManagedDir)
      .catch(() => {});
  }, []);

  const families = useMemo(() => [...familiesFor(fonts, tags).values()], [fonts, tags]);

  const systemFamilies = useMemo(
    () => families.filter((f) => f.faces.some((face) => face.source === "system")),
    [families],
  );
  const managedFamilies = useMemo(
    () =>
      families.filter((f) =>
        f.faces.some(
          (face) =>
            face.source === "managed" &&
            !settings.extraDirs.some((dir) => under(face.path, dir)),
        ),
      ),
    [families, settings.extraDirs],
  );

  const dirStats = useCallback(
    (dir: string, onlyUser: boolean) =>
      countsFor(
        families.filter((f) =>
          f.faces.some((face) => under(face.path, dir) && (!onlyUser || face.source === "user")),
        ),
      ),
    [families],
  );

  const totals = useMemo(() => {
    const news = families.filter((f) => !f.active).length;
    return {
      dirs: 2 + settings.extraDirs.length,
      faces: fonts.length,
      families: families.length,
      news,
    };
  }, [families, fonts.length, settings.extraDirs.length]);

  const addDir = async () => {
    const dir = await open({ directory: true, title: t("folders.addTitle") });
    if (!dir) return;
    if (settings.extraDirs.some((d) => norm(d) === norm(dir))) {
      toast.error(t("folders.duplicate"), dir);
      return;
    }
    setBusy(true);
    try {
      await updateSettings({ ...settings, extraDirs: [...settings.extraDirs, dir] });
    } finally {
      setBusy(false);
    }
  };

  const removeDir = async (dir: string) => {
    setBusy(true);
    try {
      await updateSettings({
        ...settings,
        extraDirs: settings.extraDirs.filter((d) => d !== dir),
      });
    } finally {
      setBusy(false);
    }
  };

  const rescanAll = async () => {
    setBusy(true);
    try {
      await rescan();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="ref-view">
      <div className="ref-inner">
        <header className="ref-head">
          <div className="ref-head-text">
            <h1 className="ref-title">{t("folders.title")}</h1>
            <p className="ref-sub">{t("folders.subtitle")}</p>
          </div>
          <div className="ref-head-actions">
            <div className="fdir-watch">
              <span>{t("folders.watch")}</span>
              <PillToggle
                on={settings.watchEnabled}
                onChange={(on) => void updateSettings({ ...settings, watchEnabled: on })}
                label={t("folders.watch")}
              />
            </div>
            <button className="ref-btn" onClick={() => void rescanAll()} disabled={busy || phase === "scanning"}>
              {busy || phase === "scanning" ? (
                <LoaderCircle size={13} strokeWidth={1.5} className="bk-spin" />
              ) : (
                <RefreshCw size={13} strokeWidth={1.5} />
              )}
              {t("folders.rescan")}
            </button>
            <button className="ref-btn primary" onClick={() => void addDir()} disabled={busy}>
              <Plus size={13} strokeWidth={1.5} />
              {t("folders.add")}
            </button>
          </div>
        </header>

        <div className="ref-body">
          <div className="ref-stats">
            <div className="ref-stat">
              <span className="ref-stat-value tabular">{totals.dirs}</span>
              <span className="ref-stat-label">{t("folders.statDirs")}</span>
            </div>
            <div className="ref-stat">
              <span className="ref-stat-value tabular">{totals.faces}</span>
              <span className="ref-stat-label">{t("folders.statFaces")}</span>
            </div>
            <div className="ref-stat">
              <span className="ref-stat-value tabular">{totals.families}</span>
              <span className="ref-stat-label">{t("folders.statFamilies")}</span>
            </div>
            <div className={`ref-stat ${totals.news > 0 ? "primary" : "dim"}`}>
              <span className="ref-stat-value tabular">{totals.news}</span>
              <span className="ref-stat-label">{t("folders.statMissing")}</span>
            </div>
          </div>

          <section className="fdir-group">
            <h2 className="fdir-group-title">{t("folders.sectionSystem")}</h2>
            <DirRow
              icon={<HardDrive size={15} strokeWidth={1.5} />}
              path="C:\Windows\Fonts"
              stats={countsFor(systemFamilies)}
              tag={t("folders.tagSystem")}
              readonly
              index={0}
            />
            {managedDir && (
              <DirRow
                icon={<FolderOpen size={15} strokeWidth={1.5} />}
                path={settings.libraryDirEnabled && settings.libraryDir ? settings.libraryDir : managedDir}
                stats={countsFor(managedFamilies)}
                tag={t("folders.tagManaged")}
                readonly
                index={1}
              />
            )}
          </section>

          <section className="fdir-group">
            <h2 className="fdir-group-title">{t("folders.sectionMine")}</h2>
            {settings.extraDirs.length === 0 ? (
              <div className="fdir-empty">
                <FolderCog size={22} strokeWidth={1.5} />
                <p>{t("folders.emptyTitle")}</p>
                <span className="fdir-empty-sub">{t("folders.emptyBody")}</span>
                <button className="ref-btn primary" onClick={() => void addDir()} disabled={busy}>
                  <Plus size={13} strokeWidth={1.5} />
                  {t("folders.add")}
                </button>
              </div>
            ) : (
              settings.extraDirs.map((dir, i) => {
                const s = dirStats(dir, false);
                return (
                  <DirRow
                    key={dir}
                    icon={<FolderCog size={15} strokeWidth={1.5} />}
                    path={dir}
                    stats={s}
                    tag={s.missing > 0 ? t("folders.tagUninstalled") : t("folders.tagAllInstalled")}
                    index={i + 2}
                    actions={
                      <>
                        <button
                          className="fdir-btn"
                          aria-label={t("folders.reveal", { dir })}
                          title={t("folders.reveal", { dir })}
                          onClick={() =>
                            revealItemInDir(dir).catch((e) =>
                              toast.error(t("toast.couldntOpenFileManager"), String(e)),
                            )
                          }
                        >
                          <FolderOpen size={13} strokeWidth={1.5} />
                        </button>
                        <button
                          className="fdir-btn danger"
                          aria-label={t("folders.remove", { dir })}
                          title={t("folders.remove", { dir })}
                          disabled={busy}
                          onClick={() => void removeDir(dir)}
                        >
                          <Trash2 size={13} strokeWidth={1.5} />
                        </button>
                      </>
                    }
                  />
                );
              })
            )}
          </section>

          <p className="fdir-hint">
            <CheckCircle2 size={13} strokeWidth={1.5} /> {t("folders.hintScan")}
            <br />
            <CircleDashed size={13} strokeWidth={1.5} /> {t("folders.hintRemove")}
          </p>
        </div>
      </div>
    </div>
  );
}
