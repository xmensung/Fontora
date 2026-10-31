import { invoke } from "@tauri-apps/api/core";

export type FontFormat = "otf" | "ttf" | "woff" | "woff2" | "unknown";
export type FontSource = "system" | "user" | "managed";
export type Classification = "serif" | "sans" | "mono" | "display" | "script" | "unknown";

export interface VariationAxis {
  tag: string;
  min: number;
  default: number;
  max: number;
}

export interface FontFace {
  id: string;
  path: string;

  previewPath: string | null;
  faceIndex: number;
  family: string;
  style: string;
  postscriptName: string | null;
  foundry: string | null;
  license: string | null;
  licenseUrl: string | null;
  format: FontFormat;
  isVariable: boolean;
  axes: VariationAxis[];
  weight: number;
  italic: boolean;
  monospaced: boolean;
  classification: Classification;
  scripts: string[];
  features: string[];
  glyphCount: number;
  fileSize: number;
  /** 文件修改时间（Unix 秒）。「加入时间」排序用它。 */
  modified: number;
  source: FontSource;
  deactivatable: boolean;
  active: boolean;
}

export interface TrashEntry {
  id: string;
  originalPath: string;
  trashedPath: string;
  family: string;
  trashedAt: number;
}

export interface InstallResult {
  installed: FontFace[];
  errors: string[];
  duplicates: string[];
}

export interface ScanProgress {
  done: number;
  total: number;
}

export interface InstallProgress {
  file: string;
  done: number;
  total: number;
  ok: boolean;
  error: string | null;
}

export type AdobeApp = "photoshop" | "illustrator";

export interface AppSettings {
  extraDirs: string[];
  watchEnabled: boolean;
  autoActivateImports: boolean;
  libraryDir: string | null;
  libraryDirEnabled: boolean;
  affinityEnabled: boolean;
  affinityDeactivateOnQuit: boolean;
  onlineFontsEnabled: boolean;
}

export interface OnlineFamily {
  id: string;
  family: string;
  category: string;
  weights: number[];
  styles: string[];
  subsets: string[];
  variable: boolean;
  license: string;
  lastModified: string;
}

export interface OnlineCatalogue {
  fetchedAt: number;
  fromCache: boolean;
  families: OnlineFamily[];
}

export interface OnlineFile {
  name: string;
  style: string;
  weight: number;
  filename: string;
  postScriptName: string;
  fullName: string;
  copyright: string;
}

export interface OnlineFamilyDetail {
  id: string;
  family: string;
  designer: string;
  category: string;
  license: string;
  licenseName: string;
  licenseUrl: string;
  reservedFontName: string | null;
  copyright: string;
  files: OnlineFile[];
  licenseText: string;
  sourceDir: string;
}

export interface OnlineDownloadResult {
  family: string;
  paths: string[];
  licensePath: string;
}

export interface OnlineProgress {
  id: string;
  file: string;
  done: number;
  total: number;
}

export interface AffinityConnection {
  reachable: boolean;
  version: string | null;
  docCount: number;
  error: string | null;
}

export type InstallMode = "link" | "move";

// --- font detail page -------------------------------------------------------

export interface LocalizedName {
  lang: string;
  langCode: string;
  family: string | null;
  style: string | null;
}

export interface IntroBlock {
  label: string;
  body: string;
}

export interface EmbedFlag {
  bit: number;
  label: string;
  detail: string;
  on: boolean;
  grants: boolean;
}

export interface LicenseInfo {
  headline: string;
  fsType: number;
  flags: EmbedFlag[];
  allow: string[];
  deny: string[];
  copyright: string | null;
  trademark: string | null;
  licenseText: string | null;
  licenseUrl: string | null;
  vendorUrl: string | null;
}

export interface CoverageRow {
  id: string;
  label: string;
  group: "simp" | "both" | "trad" | "block";
  covered: number;
  total: number;
  stars: number | null;
}

export interface Coverage {
  listed: CoverageRow[];
  blocks: CoverageRow[];
}

export interface CjkBlockCount {
  name: string;
  start: number;
  end: number;
  count: number;
}

export interface CjkStats {
  totalChars: number;
  han: number;
  hanBasic: number;
  hanExtA: number;
  hanExtB: number;
  hanExtC: number;
  hanExtD: number;
  hanExtE: number;
  hanExtF: number;
  hanExtG: number;
  hanExtH: number;
  hanExtI: number;
  hanCompat: number;
  kana: number;
  hangul: number;
  latin: number;
  fullwidth: number;
  blocks: CjkBlockCount[];
}

export interface SampleLine {
  label: string;
  text: string;
  size: number;
}

/** 一个重复字体组（与 Rust 端 dupe::DupeGroup 对应）。 */
export interface DupeFile {
  path: string;
  fileName: string;
  family: string;
  style: string;
  size: number;
  modified: number;
  source: string;
  system: boolean;
  deletable: boolean;
  kept: boolean;
  identical: boolean;
}

export interface DupeGroup {
  id: string;
  /** identical = 字节完全相同；sameName = 同名不同版本。 */
  kind: "identical" | "sameName";
  label: string;
  totalSize: number;
  reclaimable: number;
  files: DupeFile[];
}

export interface FontDetail {
  family: string;
  style: string;
  postscriptName: string | null;
  path: string;
  format: FontFormat;
  fileSize: number;
  modified: number;
  sha256: string | null;
  glyphCount: number;
  weight: number;
  italic: boolean;
  monospaced: boolean;
  classification: string;
  facesInFile: number;
  isVariable: boolean;
  axes: VariationAxis[];
  version: string | null;
  designer: string | null;
  designerUrl: string | null;
  manufacturer: string | null;
  description: string | null;
  uniqueId: string | null;
  variationPrefix: string | null;
  created: string | null;
  modifiedDate: string | null;
  names: LocalizedName[];
  scripts: string[];
  features: string[];
  intro: IntroBlock[];
  license: LicenseInfo;
  coverage: Coverage;
  cjk: CjkStats;
  samples: SampleLine[];
}

/** 一次「导出字体」的结果（拆包 / 改名 / 子集化共用）。 */
export interface FaceExport {
  path: string;
  family: string;
  style: string;
  byteSize: number;
  facesInFile: number;
  renamed: boolean;
}

/** 子集化的字符集预设。`all` = 不裁字符，只转格式。 */
export type SubsetCharset = "latin" | "zh" | "zhFull" | "text" | "all";
export type SubsetFormat = "ttf" | "woff" | "woff2";

export interface RefFile {
  path: string;
  fileName: string;
  size: number;
  modified: number;
  /** pdf / web / zip / binary */
  kind: string;
  installed: string[];
  missing: string[];
}

export interface RefFont {
  name: string;
  files: number;
  installed: boolean;
}

export interface ReferenceReport {
  root: string;
  scanned: number;
  elapsedMs: number;
  files: RefFile[];
  fonts: RefFont[];
  missingCount: number;
  installedCount: number;
}

/** 字符地图里的一段 Unicode 区段。 */
export interface CharBlock {
  id: string;
  label: string;
  start: number;
  end: number;
  /** 这款字体在这个区段实际有多少个字符。 */
  covered: number;
}

export interface GlyphName {
  cp: number;
  name: string;
}

export interface CharMap {
  total: number;
  /** 有序闭区间 [start, end]，用二分查找判断某个码位有没有。 */
  ranges: [number, number][];
  blocks: CharBlock[];
  glyphNames: GlyphName[];
  hasLigatures: boolean;
  outside: number;
}

export type HealthLevel = "error" | "warn" | "info";

export interface HealthIssue {
  level: HealthLevel;
  title: string;
  detail: string;
}

export interface HealthTable {
  tag: string;
  length: number;
  checksumOk: boolean;
}

export interface Health {
  score: number;
  issues: HealthIssue[];
  tables: HealthTable[];
  faceCount: number;
  faceIndex: number;
}

/** 一款字体的使用计数。三类事件分开记，互不影响。 */
export interface UsageRow {
  family: string;
  views: number;
  lastView: number;
  installs: number;
  lastInstall: number;
  refs: number;
  lastRef: number;
  firstSeen: number;
}

/** 归档时交给后端的一个文件（同一个 TTC 的多个字重只会传一次路径）。 */
export interface OrganizeItem {
  path: string;
  family: string;
  foundry: string | null;
  scripts: string[];
  format: string;
  classification: string;
  size: number;
  source: string;
  active: boolean;
}

export interface OrganizeEntry {
  path: string;
  target: string;
  folder: string;
  action: string;
  reason: string;
  size: number;
  family: string;
}

export interface OrganizeFolder {
  name: string;
  count: number;
  bytes: number;
}

export interface OrganizePlan {
  dest: string;
  key: string;
  mode: string;
  entries: OrganizeEntry[];
  ready: number;
  skipped: number;
  bytes: number;
  collisions: number;
  folders: OrganizeFolder[];
}

export interface OrganizeOutcome {
  family: string;
  path: string;
  target: string;
  status: string;
  message: string;
}

export interface OrganizeResult {
  dest: string;
  done: number;
  failed: number;
  bytes: number;
  outcomes: OrganizeOutcome[];
}

/** 导出清单/字体包里的一行 = 一个字重。后半段由后端从索引补齐。 */
export interface ExportRow {
  family: string;
  style: string;
  postscriptName: string | null;
  path: string;
  faceIndex: number;
  format: string;
  weight: number;
  italic: boolean;
  variable: boolean;
  classification: string;
  foundry: string | null;
  license: string | null;
  licenseUrl: string | null;
  glyphCount: number;
  fileSize: number;
  source: string;
  active: boolean;
  tags: string[];
  collections: string[];
  favorite: boolean;
  /** 1–5，0 = 没评分。 */
  rating: number;
  note: string | null;
  fileName: string;
  sha256: string | null;
  totalChars: number | null;
  han: number | null;
}

export interface CatalogExport {
  path: string;
  rows: number;
  bytes: number;
}

/** 迁移包里能装回去的东西，导出/导入共用。 */
export interface BundleManifest {
  format: string;
  version: number;
  app: string;
  exportedAt: number;
  machine: string;
  families: number;
  faces: number;
  includesFonts: boolean;
  includesUsage: boolean;
  files: number;
}

export interface BundleCounts {
  tags: number;
  tagItems: number;
  collections: number;
  favorites: number;
  ratings: number;
  aliases: number;
  notes: number;
}

export interface ImportPlan {
  manifest: BundleManifest;
  counts: BundleCounts;
  /** 包里有、本机暂时没有的家族名（最多 24 个）。 */
  unknown: string[];
  unknownTotal: number;
  stalePaths: number;
}

export interface ImportResult {
  mode: string;
  tags: number;
  collections: number;
  favorites: number;
  ratings: number;
  aliases: number;
  notes: number;
  settings: boolean;
  usage: number;
  fontsWritten: number;
  fontsSkipped: number;
  stalePaths: number;
  droppedDirs: number;
}

export interface BundleExport {
  path: string;
  bytes: number;
  families: number;
  files: number;
}

/** 一条操作日志。`detail` 是后端给的结构化数据，文案在前端拼。 */
export interface AuditEntry {
  id: number;
  at: number;
  kind: string;
  count: number;
  detail: Record<string, unknown>;
}

export interface PackageExport {
  path: string;
  fonts: number;
  webFonts: number;
  bytes: number;
  errors: string[];
}

export const ipc = {
  getSettings: () => invoke<AppSettings>("get_settings"),
  setSettings: (settings: AppSettings) => invoke<void>("set_settings", { settings }),
  defaultLibraryDir: () => invoke<string>("default_library_dir"),
  affinityConnection: () => invoke<AffinityConnection>("affinity_connection"),
  affinitySessionActivate: (paths: string[]) =>
    invoke<string[]>("affinity_session_activate", { paths }),
  onlineCatalogue: (force: boolean) => invoke<OnlineCatalogue>("online_catalogue", { force }),
  onlineFamily: (id: string) => invoke<OnlineFamilyDetail>("online_family", { id }),
  onlineDownload: (id: string) => invoke<OnlineDownloadResult>("online_download", { id }),
  onlinePreview: (id: string) => invoke<string>("online_preview", { id }),
  onlineClearStaging: (id: string) => invoke<void>("online_clear_staging", { id }),
  onlineHosts: () => invoke<string[]>("online_hosts"),
  scanFonts: () => invoke<FontFace[]>("scan_fonts"),
  setFontActive: (path: string, active: boolean) =>
    invoke<void>("set_font_active", { path, active }),
  setFontsActive: (paths: string[], active: boolean) =>
    invoke<void>("set_fonts_active", { paths, active }),
  setFontsActiveSession: (paths: string[]) =>
    invoke<void>("set_fonts_active_session", { paths }),
  installFonts: (paths: string[], existing: string[], mode: InstallMode) =>
    invoke<InstallResult>("install_fonts", { paths, existing, mode }),
  uninstallFont: (path: string, family: string) =>
    invoke<TrashEntry>("uninstall_font", { path, family }),
  listTrash: () => invoke<TrashEntry[]>("list_trash"),
  restoreFromTrash: (entryId: string) => invoke<void>("restore_from_trash", { entryId }),
  deleteTrashEntry: (entryId: string) => invoke<void>("delete_trash_entry", { entryId }),
  emptyTrash: () => invoke<void>("empty_trash"),
  getTags: () => invoke<Record<string, string[]>>("get_tags"),
  setTags: (family: string, tags: string[]) => invoke<void>("set_tags", { family, tags }),
  importTags: (entries: Record<string, string[]>) =>
    invoke<number>("import_tags", { entries }),
  getFavorites: () => invoke<string[]>("get_favorites"),
  getNotes: () => invoke<Record<string, string>>("get_notes"),
  setNote: (family: string, note: string) => invoke<void>("set_note", { family, note }),
  setFavorite: (family: string, favorite: boolean) =>
    invoke<void>("set_favorite", { family, favorite }),
  getRatings: () => invoke<Record<string, number>>("get_ratings"),
  setRating: (family: string, score: number) =>
    invoke<void>("set_rating", { family, score }),
  getAliases: () => invoke<Record<string, string[]>>("get_aliases"),
  setAliases: (family: string, aliases: string[]) =>
    invoke<void>("set_aliases", { family, aliases }),
  /** 一批名字 → 拼音检索键（小写 ASCII）。 */
  pinyinKeys: (names: string[]) => invoke<Record<string, string[]>>("pinyin_keys", { names }),
  getCharset: (path: string, faceIndex: number) =>
    invoke<number[]>("get_charset", { path, faceIndex }),
  fontDetail: (path: string, faceIndex: number) =>
    invoke<FontDetail>("font_detail", { path, faceIndex }),
  fontDuplicates: () => invoke<DupeGroup[]>("font_duplicates"),
  getFeatures: (path: string, faceIndex: number) =>
    invoke<string[]>("get_features", { path, faceIndex }),
  readTextFile: (path: string) => invoke<string>("read_text_file", { path }),
  writeBinaryFile: (path: string, data: number[]) =>
    invoke<void>("write_binary_file", { path, data }),
  getCollections: () => invoke<Record<string, string[]>>("get_collections"),
  setCollection: (name: string, families: string[]) =>
    invoke<void>("set_collection", { name, families }),
  deleteCollection: (name: string) => invoke<void>("delete_collection", { name }),
  renameCollection: (from: string, to: string) =>
    invoke<void>("rename_collection", { from, to }),
  exportFont: (src: string, dest: string) => invoke<void>("export_font", { src, dest }),
  exportFontFace: (path: string, faceIndex: number, dest: string) =>
    invoke<FaceExport>("export_font_face", { path, faceIndex, dest }),
  exportFontFaceRenamed: (
    path: string,
    faceIndex: number,
    dest: string,
    family: string,
    style: string,
  ) => invoke<FaceExport>("export_font_face_renamed", { path, faceIndex, dest, family, style }),
  exportFontCollection: (path: string, destDir: string) =>
    invoke<FaceExport[]>("export_font_collection", { path, destDir }),
  exportFontSubset: (
    path: string,
    faceIndex: number,
    dest: string,
    charset: SubsetCharset,
    text: string,
    format: SubsetFormat,
  ) =>
    invoke<FaceExport>("export_font_subset", { path, faceIndex, dest, charset, text, format }),
  fontFileFaces: (path: string) => invoke<number>("font_file_faces", { path }),
  fontReferences: (root: string) => invoke<ReferenceReport>("font_references", { root }),
  fontCharmap: (path: string, faceIndex: number) =>
    invoke<CharMap>("font_charmap", { path, faceIndex }),
  fontHealth: (path: string, faceIndex: number) =>
    invoke<Health>("font_health", { path, faceIndex }),
  fontUsage: () => invoke<UsageRow[]>("font_usage"),
  fontUsageTouch: (families: string[], kind: string) =>
    invoke<number>("font_usage_touch", { families, kind }),
  fontUsageClear: () => invoke<void>("font_usage_clear"),
  organizeScan: (dir: string) => invoke<OrganizeItem[]>("organize_scan", { dir }),
  organizePlan: (items: OrganizeItem[], dest: string, key: string, mode: string) =>
    invoke<OrganizePlan>("organize_plan", { items, dest, key, mode }),
  organizeApply: (items: OrganizeItem[], dest: string, key: string, mode: string) =>
    invoke<OrganizeResult>("organize_apply", { items, dest, key, mode }),
  bundleExport: (dest: string, rows: ExportRow[], withFiles: boolean, withUsage: boolean) =>
    invoke<BundleExport>("bundle_export", { dest, rows, withFiles, withUsage }),
  bundleInspect: (path: string, known: string[]) =>
    invoke<ImportPlan>("bundle_inspect", { path, known }),
  bundleImport: (path: string, mode: string) =>
    invoke<ImportResult>("bundle_import", { path, mode }),
  auditLog: (limit: number) => invoke<AuditEntry[]>("audit_log", { limit }),
  auditClear: () => invoke<number>("audit_clear"),
  exportCatalog: (dest: string, format: string, rows: ExportRow[], chinese: boolean) =>
    invoke<CatalogExport>("export_catalog", { dest, format, rows, chinese }),
  exportFontPackage: (
    dest: string,
    name: string,
    rows: ExportRow[],
    withFiles: boolean,
    web: boolean,
    css: boolean,
  ) => invoke<PackageExport>("export_font_package", { dest, name, rows, withFiles, web, css }),
  fontFaceCss: (rows: ExportRow[], web: boolean) =>
    invoke<string>("font_face_css", { rows, web }),
  writeTextFile: (path: string, content: string) =>
    invoke<void>("write_text_file", { path, content }),
  exportFonts: (paths: string[], destDir: string) =>
    invoke<number>("export_fonts", { paths, destDir }),
  adobeAvailable: () => invoke<boolean>("adobe_available"),
  applyFontInApp: (app: AdobeApp, postscriptName: string, label: string) =>
    invoke<string>("apply_font_in_app", { app, postscriptName, label }),
  affinityLink: () => invoke<AffinityConnection | null>("affinity_link"),
  setAutostart: (enabled: boolean) => invoke<void>("set_autostart", { enabled }),
  getAutostart: () => invoke<boolean>("get_autostart"),
  getPrefs: () => invoke<Record<string, unknown> | null>("get_prefs"),
  setPrefs: (prefs: Record<string, unknown>) => invoke<void>("set_prefs", { prefs }),
  fontsWithChar: (paths: string[], codepoint: number) =>
    invoke<string[]>("fonts_with_char", { paths, codepoint }),
  setUiScale: (scale: number | null) => invoke<number>("set_ui_scale", { scale }),
};
