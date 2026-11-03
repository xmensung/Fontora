import type { CjkStats, Coverage, FontDetail } from "./ipc";
import { normalizeTag } from "./tagTaxonomy";

/**
 * 自动标签建议：基于字体文件的客观元数据（字重、斜体、字宽、脚本覆盖、
 * CJK 字数、OpenType 特性）推导多标签，不依赖用户手动数据。
 * 返回值是「建议」，只用于补全 —— 用户已有的手动标签一律保留。
 */

interface SuggestInput {
  family: string;
  classification: string;
  weight: number;
  italic: boolean;
  monospaced: boolean;
  isVariable: boolean;
  scripts: string[];
  features: string[];
  coverage?: Coverage;
  cjk?: CjkStats;
}

const CALLIGRAPHY_RE = /楷|行|草|隶|篆|魏碑|书法|手写|硬笔|毛笔|calligraph|brush|script|hand/i;
const KAI_RE = /楷|kai/i;
const SONG_RE = /宋|明|sung|song|serif|mincho/i;
const HEI_RE = /黑|等线|hei|gothic|sans/i;

export function suggestTags(f: SuggestInput): string[] {
  const out = new Set<string>();
  const name = f.family;
  const cls = f.classification.toLowerCase();
  const hasHan = (f.cjk?.han ?? 0) > 0;
  // scripts 里报的是 `cjk`（parser::script_coverage 的键名），不是 "Han"。
  const isCJK = f.scripts.includes("cjk") || hasHan;

  // ---- 书体源流 ----
  if (isCJK) {
    if (KAI_RE.test(name)) out.add("楷体");
    if (SONG_RE.test(name)) out.add("宋体");
    if (/仿宋|fangsong/i.test(name)) out.add("仿宋");
    if (HEI_RE.test(name)) out.add("黑体");
    if (/圆|yuanti|round/i.test(name)) out.add("圆体");
    if (/隶|lishu/i.test(name)) out.add("隶书");
    if (/篆|zhuanshu|seal/i.test(name)) out.add("篆书");
    if (/行书|行楷|xing/i.test(name) && CALLIGRAPHY_RE.test(name)) out.add("行书");
    if (/草|caoshu|cursive/i.test(name)) out.add("草书");
  } else {
    if (cls === "serif") out.add("西文衬线");
    if (cls === "sans") out.add("西文无衬线");
    if (cls === "script") out.add("手写体");
    if (cls === "display") out.add("装饰体");
  }

  // ---- 功能与气质 ----
  if (CALLIGRAPHY_RE.test(name)) out.add("书法手写");
  if (isCJK && !CALLIGRAPHY_RE.test(name) && (cls === "serif" || SONG_RE.test(name))) {
    out.add("正文");
    out.add("印刷出版");
  }
  if (isCJK && HEI_RE.test(name)) out.add("屏幕UI");
  if (!isCJK && cls === "sans") out.add("屏幕UI");
  if (!isCJK && cls === "serif") {
    out.add("正文");
    out.add("印刷出版");
  }
  if (f.weight >= 700) out.add("标题");
  if (f.weight >= 800 || cls === "display") {
    out.add("标题");
    out.add("品牌海报");
  }
  if (cls === "display") out.add("艺术展示");
  if (/儿童|卡通|可爱|kids|cartoon|comic|rounded/i.test(name)) out.add("活泼儿童");
  if (/复古|怀旧|retro|vintage|oldstyle/i.test(name)) out.add("复古怀旧");
  if (!isCJK && cls === "sans" && f.weight >= 400 && f.weight < 700 && /grot|neo|corp|offic/i.test(name)) {
    out.add("商务严肃");
  }

  // ---- 特殊属性 ----
  if (f.monospaced) out.add("等宽");
  if (f.isVariable) out.add("可变字体");
  if (f.italic) out.add("斜体");
  if (isCJK && f.scripts.includes("latin")) out.add("中英混排");
  if (f.coverage?.listed.some((r) => r.group === "trad" && r.stars !== null && r.stars >= 2)) {
    out.add("繁体支持");
  }
  if ((f.cjk?.hanExtB ?? 0) > 0) out.add("生僻字支持");
  if (f.features.some((x) => x === "smcp" || x === "onum" || x === "frac")) out.add("专业排版");
  if ((f.cjk?.han ?? 0) >= 6000) out.add("大字库");

  return [...out].map(normalizeTag);
}

/** 取一款字体用于判定的代表字面：优先 detail（更全），否则用族级字段。 */
export function suggestFromDetail(family: string, detail: FontDetail): string[] {
  return suggestTags({
    family,
    classification: detail.classification,
    weight: detail.weight,
    italic: detail.italic,
    monospaced: detail.monospaced,
    isVariable: detail.isVariable,
    scripts: detail.scripts,
    features: detail.features,
    coverage: detail.coverage,
    cjk: detail.cjk,
  });
}
