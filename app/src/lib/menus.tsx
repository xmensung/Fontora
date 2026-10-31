import { open, save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  Boxes,
  Columns2,
  Code2,
  Copy,
  Plus,
  Download,
  FileDown,
  FileText,
  FolderOpen,
  FolderPlus,
  Image as ImageIcon,
  Package,
  PenTool,
  Power,
  Star,
  Tag,
  Timer,
  Trash2,
} from "lucide-react";
import type { MenuItem } from "../design/primitives/ContextMenu";
import { toast } from "../design/primitives/Toast";
import { allTags, familiesFor, useFontStore, type Family } from "../state/fontStore";
import { ipc } from "./ipc";
import { copyFontFaceCss, scopeLabel } from "./exporter";
import { exportSpecimen } from "./specimen";
import { t } from "./i18n";

export async function exportFamily(family: Family) {
  const dir = await open({
    directory: true,
    title: t("menu.exportFamilyTitle", { name: family.name }),
  });
  if (!dir) return;
  const paths = [...new Set(family.faces.map((f) => f.path))];
  try {
    const n = await ipc.exportFonts(paths, dir);
    toast.success(
      t("toast.exported", { name: family.name }),
      t("toast.filesCopied", { count: n, dir }),
    );
  } catch (e) {
    toast.error(t("toast.exportFailed"), String(e));
  }
}

/** 导出单个字重：TTC/OTC 会真的把第 N 个 face 拆成独立文件，单字体文件则原样复制。 */
export async function exportFace(
  path: string,
  faceIndex: number,
  suggestedName: string,
  extension: string,
) {
  const dest = await save({
    defaultPath: `${suggestedName}.${extension || "ttf"}`,
    title: t("menu.exportFontFile"),
    filters: [{ name: extension.toUpperCase(), extensions: [extension || "ttf"] }],
  });
  if (!dest) return;
  try {
    const result = await ipc.exportFontFace(path, faceIndex, dest);
    toast.success(
      t("toast.fontExported"),
      `${result.path} · ${Math.round(result.byteSize / 1024)} KB`,
    );
  } catch (e) {
    toast.error(t("toast.exportFailed"), String(e));
  }
}

/** 把字体集合（TTC/OTC）里的每个 face 都拆成独立文件。 */
export async function exportCollection(path: string) {
  try {
    const faces = await ipc.fontFileFaces(path);
    if (faces <= 1) {
      toast.error(t("toast.notCollection"), t("toast.notCollectionDetail"));
      return;
    }
    const dir = await open({ directory: true, title: t("menu.exportCollectionTitle") });
    if (!dir) return;
    const results = await ipc.exportFontCollection(path, dir);
    toast.success(t("toast.collectionExported", { count: results.length }), dir);
  } catch (e) {
    toast.error(t("toast.exportFailed"), String(e));
  }
}

function copyText(text: string, what: string) {
  navigator.clipboard
    .writeText(text)
    .then(() => toast.success(t("toast.copied", { what }), undefined, "tick"))
    .catch((e) => toast.error(t("toast.couldntCopy"), String(e)));
}

async function exportFamilies(families: Family[]) {
  const dir = await open({
    directory: true,
    title: t("menu.exportFamiliesTitle", { count: families.length }),
  });
  if (!dir) return;
  const paths = [...new Set(families.flatMap((f) => f.faces.map((x) => x.path)))];
  try {
    const n = await ipc.exportFonts(paths, dir);
    toast.success(
      t("toast.exportedFamilies", { count: families.length }),
      t("toast.filesCopied", { count: n, dir }),
    );
  } catch (e) {
    toast.error(t("toast.exportFailed"), String(e));
  }
}

export async function exportFontList(families: Family[], title: string) {
  const dest = await save({
    defaultPath: `${title.replace(/[^\w-]+/g, "-").toLowerCase()}-fonts.md`,
    title: t("menu.exportListTitle"),
  });
  if (!dest) return;
  const lines = [
    `# ${t("fontlist.title", { title })}`,
    "",
    t("fontlist.sub", { count: families.length }),
    "",
    ...families.flatMap((f) => [
      `## ${f.name}`,
      `- ${t("fontlist.styles")}: ${f.faces.map((x) => x.style).join(", ")}`,
      `- ${t("fontlist.format")}: ${f.formats.join(", ").toUpperCase()}${
        f.isVariable ? ` (${t("fontlist.variable")})` : ""
      }`,
      ...(f.foundry ? [`- ${t("fontlist.foundry")}: ${f.foundry}`] : []),
      "",
    ]),
  ];
  try {
    await ipc.writeTextFile(dest, lines.join("\n"));
    toast.success(t("toast.fontListExported"), dest);
  } catch (e) {
    toast.error(t("toast.exportFailed"), String(e));
  }
}

function buildBulkMenu(names: string[]): MenuItem[] {
  const s = useFontStore.getState();
  const familyMap = familiesFor(s.fonts, s.tags);
  const families = names
    .map((n) => familyMap.get(n))
    .filter((f): f is Family => Boolean(f));
  const collectionNames = Object.keys(s.collections).sort((a, b) => a.localeCompare(b));
  const n = names.length;

  return [
    { kind: "heading", label: t("menu.selectedCount", { count: n }) },
    {
      label: t("menu.compareCount", { count: Math.min(n, 4) }),
      icon: <Columns2 size={14} strokeWidth={1.5} />,
      disabled: n < 2,
      action: () => s.openCompare(names),
    },
    { kind: "separator" },
    {
      label: t("menu.activateCount", { count: n }),
      icon: <Power size={14} strokeWidth={1.5} />,
      action: () => void s.setFamiliesActiveBulk(names, true),
    },
    {
      label: t("menu.deactivateCount", { count: n }),
      icon: <Power size={14} strokeWidth={1.5} />,
      action: () => void s.setFamiliesActiveBulk(names, false),
    },
    {
      kind: "rating",
      label: t("menu.ratingCount", { count: n }),
      value: names.every((f) => (s.ratings[f] ?? 0) === (s.ratings[names[0]] ?? 0))
        ? (s.ratings[names[0]] ?? 0)
        : 0,
      action: (score) => s.setRatingMany(names, score),
    },
    { kind: "separator" },
    { kind: "heading", label: t("menu.addAllToCollection") },
    ...collectionNames.map(
      (name): MenuItem => ({
        kind: "check",
        label: name,
        checked: names.every((f) => (s.collections[name] ?? []).includes(f)),
        action: () => {
          const current = s.collections[name] ?? [];
          const all = names.every((f) => current.includes(f));
          const next = all
            ? current.filter((f) => !names.includes(f))
            : [...new Set([...current, ...names])];
          void ipc.setCollection(name, next).then(() =>
            useFontStore.setState({
              collections: { ...useFontStore.getState().collections, [name]: next },
            }),
          );
        },
      }),
    ),
    { kind: "separator" },
    { kind: "heading", label: t("menu.tagAll") },
    ...[...allTags(s.tags).keys()].map(
      (tag): MenuItem => ({
        kind: "check",
        label: tag,
        checked: names.every((f) => (s.tags[f] ?? []).includes(tag)),
        action: () => void s.applyTagToFamilies(tag, names),
      }),
    ),
    {
      label: t("menu.newTag"),
      icon: <Plus size={14} strokeWidth={1.5} />,
      action: () => s.setBulkTagFor(names),
    },
    { kind: "separator" },
    {
      label: t("menu.exportFamiliesCount", { count: n }),
      icon: <Download size={14} strokeWidth={1.5} />,
      action: () => void exportFamilies(families),
    },
    {
      label: t("menu.exportFontList"),
      icon: <FileText size={14} strokeWidth={1.5} />,
      action: () => void exportFontList(families, t("menu.selection")),
    },
    {
      label: t("menu.exportData"),
      icon: <Package size={14} strokeWidth={1.5} />,
      action: () => s.openExport(names, scopeLabel(names)),
    },
    {
      label: t("menu.moveCountToTrash", { count: n }),
      icon: <Trash2 size={14} strokeWidth={1.5} />,
      danger: true,
      action: () => {
        for (const f of names) void s.uninstallFamily(f);
      },
    },
  ];
}

export function buildFamilyMenu(family: Family): MenuItem[] {
  const s = useFontStore.getState();
  if (s.selection.length > 1 && s.selection.includes(family.name)) {
    return buildBulkMenu(s.selection);
  }
  const lead = family.faces[0];
  const collectionNames = Object.keys(s.collections).sort((a, b) =>
    a.localeCompare(b),
  );
  const canUninstall = family.faces.some((f) => f.source !== "system");

  const favorite = s.favorites.includes(family.name);
  const items: MenuItem[] = [
    {
      label: t("menu.openDetail"),
      icon: <FileText size={14} strokeWidth={1.5} />,
      action: () => useFontStore.getState().openFontDetail(family.name),
    },
    { kind: "separator" },
    {
      // 对未安装的字体，「启用」的实质就是写注册表把它装进系统，文件不动。
      // 所以这里换个说法，别让人以为要复制文件。
      label: t(family.active ? "menu.deactivate" : "menu.installToSystem"),
      icon: <Power size={14} strokeWidth={1.5} />,
      disabled: !family.deactivatable,
      action: () => void s.setFamilyActive(family.name, !family.active),
    },
    ...(!family.active && family.deactivatable
      ? [
          {
            label: t("menu.activateUntilClose"),
            icon: <Timer size={14} strokeWidth={1.5} />,
            action: () => void useFontStore.getState().activateFamilySession(family.name),
          } satisfies MenuItem,
        ]
      : []),
    {
      label: t(favorite ? "menu.removeFromFavorites" : "menu.addToFavorites"),
      icon: <Star size={14} strokeWidth={1.5} />,
      action: () => void s.toggleFavorite(family.name),
    },
    {
      kind: "rating",
      label: t("menu.rating"),
      value: s.ratings[family.name] ?? 0,
      action: (score) => s.setRating(family.name, score),
    },
    {
      label: t("menu.aliases"),
      icon: <Tag size={14} strokeWidth={1.5} />,
      action: () => s.openAlias(family.name),
    },
    ...(s.adobeAvailable
      ? [
          { kind: "separator" } satisfies MenuItem,
          ...(["photoshop", "illustrator"] as const).map(
            (app): MenuItem => ({
              label: t("menu.applyInApp", { app: t(`adobe.${app}`) }),
              icon: <PenTool size={14} strokeWidth={1.5} />,
              action: () => void useFontStore.getState().applyFamilyInApp(family.name, app),
            }),
          ),
        ]
      : []),
    { kind: "separator" },
    { kind: "heading", label: t("menu.collections") },
    ...collectionNames.map(
      (name): MenuItem => ({
        kind: "check",
        label: name,
        checked: (s.collections[name] ?? []).includes(family.name),
        action: () => void s.toggleFamilyInCollection(name, family.name),
      }),
    ),
    {
      label: t("menu.newCollection"),
      icon: <FolderPlus size={14} strokeWidth={1.5} />,
      action: () => useFontStore.setState({ pendingCollectionFor: family.name }),
    },
    { kind: "separator" },
    {
      label: t("menu.exportFamily"),
      icon: <Download size={14} strokeWidth={1.5} />,
      action: () => void exportFamily(family),
    },
    {
      label: t("menu.exportOneFace"),
      icon: <FileDown size={14} strokeWidth={1.5} />,
      action: () =>
        void exportFace(
          lead.path,
          lead.faceIndex,
          family.faces.length > 1 ? `${family.name} ${lead.style}` : family.name,
          lead.format,
        ),
    },
    {
      label: t("menu.unpackCollection"),
      icon: <Boxes size={14} strokeWidth={1.5} />,
      action: () => void exportCollection(lead.path),
    },
    {
      label: t("menu.exportSpecimen"),
      icon: <ImageIcon size={14} strokeWidth={1.5} />,
      action: () => void exportSpecimen(family, useFontStore.getState().sampleText),
    },
    {
      label: t("menu.exportData"),
      icon: <Package size={14} strokeWidth={1.5} />,
      action: () => s.openExport([family.name], family.name),
    },
    {
      label: t("menu.copyFontFaceCss"),
      icon: <Code2 size={14} strokeWidth={1.5} />,
      action: () => void copyFontFaceCss([family.name], false),
    },
    {
      label: t("menu.copyFamilyName"),
      icon: <Copy size={14} strokeWidth={1.5} />,
      action: () => copyText(family.name, t("copy.familyName")),
    },
    {
      label: t("menu.copyFilePath"),
      icon: <Copy size={14} strokeWidth={1.5} />,
      action: () => copyText(lead.path, t("copy.filePath")),
    },
    {
      label: t("menu.showInFileManager"),
      icon: <FolderOpen size={14} strokeWidth={1.5} />,
      action: () =>
        revealItemInDir(lead.path).catch((e) =>
          toast.error(t("toast.couldntOpenFileManager"), String(e)),
        ),
    },
  ];

  if (canUninstall) {
    items.push(
      { kind: "separator" },
      {
        label: t("menu.moveToTrash"),
        icon: <Trash2 size={14} strokeWidth={1.5} />,
        danger: true,
        action: () => void s.uninstallFamily(family.name),
      },
    );
  }
  return items;
}
