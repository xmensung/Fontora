/**
 * 缺字检测的前端胶水。
 *
 * 判定本身在 Rust 侧（`check_glyphs`），这里只负责两件事：
 * 把预览文本切成「有字 / 缺字」的连续段，以及统计缺了几个。
 */

export interface MissingInfo {
  /** 该字面缺的码点集合 */
  set: Set<number>;
  /** 参与判定的字符数 */
  checked: number;
}

/** 码点去重（跟 Rust 侧口径保持一致：去空白、去控制符、去变体选择符）。 */
export function signature(text: string): number[] {
  const out = new Set<number>();
  for (const ch of text) {
    if (/\s/.test(ch)) continue;
    const cp = ch.codePointAt(0);
    if (cp === undefined) continue;
    if (cp < 0x20 || cp === 0x7f) continue;
    if (cp >= 0xfe00 && cp <= 0xfe0f) continue;
    if (cp >= 0xe0100 && cp <= 0xe01ef) continue;
    out.add(cp);
  }
  return [...out].sort((a, b) => a - b);
}

export interface Run {
  text: string;
  missing: boolean;
}

/**
 * 按「缺/不缺」把文本切成连续段。
 * 用 for...of 遍历是必要的：中文扩展区的字是代理对，按 UTF-16 下表切会切坏。
 */
export function runsOf(text: string, missing: MissingInfo | undefined): Run[] {
  if (!missing || missing.set.size === 0) return [{ text, missing: false }];
  const out: Run[] = [];
  let buf = "";
  let bufMissing: boolean | null = null;
  for (const ch of text) {
    const cp = ch.codePointAt(0)!;
    const miss = missing.set.has(cp);
    if (bufMissing === null) {
      bufMissing = miss;
    } else if (bufMissing !== miss) {
      out.push({ text: buf, missing: bufMissing });
      buf = "";
      bufMissing = miss;
    }
    buf += ch;
  }
  if (buf) out.push({ text: buf, missing: bufMissing === true });
  return out;
}

/** 缺字的展示形态：多个字连在一起时列成「山、水、月」。 */
export function missingLabel(set: Set<number>, limit = 6): string {
  const chars = [...set]
    .sort((a, b) => a - b)
    .slice(0, limit)
    .map((cp) => String.fromCodePoint(cp));
  return chars.join(" ");
}
