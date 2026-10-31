import { familiesFor, useFontStore, type Family } from "../state/fontStore";
import { toast } from "../design/primitives/Toast";
import { t } from "./i18n";
import { ipc, type ExportRow } from "./ipc";

/** 名字里带这些字符时没法当文件名，统一换成下划线。 */
export function safeName(name: string): string {
  return name.replace(/[\\/:*?"<>|]/g, "_").trim() || "字体包";
}

function rowsForFamily(family: Family, ctx: Ctx): ExportRow[] {
  return family.faces.map((face) => ({
    family: family.name,
    style: face.style,
    postscriptName: face.postscriptName,
    path: face.path,
    faceIndex: face.faceIndex,
    format: face.format,
    weight: face.weight,
    italic: face.italic,
    variable: face.isVariable,
    classification: face.classification,
    foundry: face.foundry,
    license: face.license,
    licenseUrl: face.licenseUrl,
    glyphCount: face.glyphCount,
    fileSize: face.fileSize,
    source: face.source,
    active: face.active,
    tags: ctx.tags[family.name] ?? [],
    collections: Object.entries(ctx.collections)
      .filter(([, names]) => names.includes(family.name))
      .map(([name]) => name),
    favorite: ctx.favorites.includes(family.name),
    rating: ctx.ratings[family.name] ?? 0,
    note: ctx.notes[family.name] ?? null,
    // 后端会补：文件名、SHA-256、字符数、汉字数
    fileName: "",
    sha256: null,
    totalChars: null,
    han: null,
  }));
}

interface Ctx {
  tags: Record<string, string[]>;
  collections: Record<string, string[]>;
  favorites: string[];
  notes: Record<string, string>;
  ratings: Record<string, number>;
}

/** 当前库状态 → 导出用的行。家族名不存在时静默跳过。 */
export function rowsFor(names: string[]): ExportRow[] {
  const s = useFontStore.getState();
  const map = familiesFor(s.fonts, s.tags);
  const ctx: Ctx = {
    tags: s.tags,
    collections: s.collections,
    favorites: s.favorites,
    notes: s.notes,
    ratings: s.ratings,
  };
  return names
    .map((name) => map.get(name))
    .filter((f): f is Family => Boolean(f))
    .flatMap((family) => rowsForFamily(family, ctx));
}

/** 导出面板标题里显示的作用范围。 */
export function scopeLabel(names: string[]): string {
  return names.length === 1 ? names[0] : t("export2.labelCount", { count: names.length });
}

/** 复制一段 @font-face：路径按包内目录写，界面上会说明要自行调整。 */
export async function copyFontFaceCss(names: string[], web: boolean) {
  try {
    const css = await ipc.fontFaceCss(rowsFor(names), web);
    await navigator.clipboard.writeText(css);
    toast.success(t("toast.copied", { what: t("copy.fontFaceCss") }), undefined, "tick");
  } catch (e) {
    toast.error(t("toast.couldntCopy"), String(e));
  }
}
