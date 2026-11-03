/**
 * 中文书体判定。
 *
 * 只认族名。为什么不认字形：判一款中文字体是宋是黑，最靠得住的是人眼，
 * 其次是族名里的关键词 —— 这款软件里 90% 的中文字体名字都带「宋/黑/楷/仿宋」，
 * 或者带 Serif/Sans（思源、Noto 系列）。剩下的落到「其他」，比瞎猜一个好。
 *
 * 顺序很关键：「仿宋」里含「宋」，所以仿宋必须先判；「行楷」里含「楷」，
 * 归到行草更准确，所以行草也在楷之前判。
 */

export type BookStyle =
  | "hei"
  | "song"
  | "kai"
  | "fangsong"
  | "yuan"
  | "lishu"
  | "zhuanshu"
  | "xingcao"
  | "other";

export const BOOK_STYLES: readonly BookStyle[] = [
  "hei",
  "song",
  "kai",
  "fangsong",
  "yuan",
  "lishu",
  "zhuanshu",
  "xingcao",
  "other",
];

const RULES: readonly [BookStyle, RegExp][] = [
  ["fangsong", /仿宋|fangsong/i],
  ["xingcao", /行书|行楷|行草|草书|xingkai|xingshu|caoshu|cursive/i],
  ["zhuanshu", /篆|zhuanshu|seal\s*script/i],
  ["lishu", /隶|隸|lishu/i],
  ["yuan", /圆体|圓體|幼圆|幼圓|圆黑|yuanti|rounded|round/i],
  ["kai", /楷|kai/i],
  ["song", /宋|明体|明體|明朝|仿宋_GB|sung|song|serif|mincho|mincho/i],
  ["hei", /黑|等线|等線|雅黑|hei|gothic|sans|pingfang|苹方/i],
];

/** 是不是中文字体（有汉字覆盖）。`scripts` 里用的是 `cjk` 这个键。 */
export function isChinese(family: { scripts: string[]; name: string }): boolean {
  if (family.scripts.includes("cjk")) return true;
  // 少数只报了 Latin 但名字一看就是中文字体的，按名字兜一下。
  return /[\u4e00-\u9fff]/.test(family.name);
}

export function bookStyleOf(family: { scripts: string[]; name: string }): BookStyle | null {
  if (!isChinese(family)) return null;
  for (const [style, re] of RULES) {
    if (re.test(family.name)) return style;
  }
  return "other";
}
