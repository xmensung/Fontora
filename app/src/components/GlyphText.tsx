import { useMemo } from "react";
import { runsOf, type MissingInfo } from "../lib/glyphs";
import { useFontStore } from "../state/fontStore";

/**
 * 取某款字体的缺字集合。
 *
 * 只有当缺字表是按「当前这段预览文字」算出来的时候才认；文本一改，
 * 旧结论立刻作废 —— 否则会出现「明明改了句子，红字还挂着」的假象。
 */
export function useMissing(name: string): Set<number> | undefined {
  const arr = useFontStore((s) => s.glyphMissing[name]);
  const glyphText = useFontStore((s) => s.glyphText);
  const sampleText = useFontStore((s) => s.sampleText);

  return useMemo(() => {
    if (!arr || arr.length === 0) return undefined;
    if (glyphText !== sampleText.trim()) return undefined;
    return new Set(arr);
  }, [arr, glyphText, sampleText]);
}

/**
 * 把预览文本渲染成「有字 / 缺字」两色。
 * 缺字只加标记、不换字体 —— 正好让人看到它掉回界面字体时的样子。
 */
export function GlyphText({ text, missing }: { text: string; missing?: Set<number> }) {
  const runs = useMemo(() => {
    const info: MissingInfo | undefined = missing ? { set: missing, checked: 0 } : undefined;
    return runsOf(text, info);
  }, [text, missing]);

  if (runs.length === 1 && !runs[0].missing) return <>{text}</>;

  return (
    <>
      {runs.map((run, i) =>
        run.missing ? (
          <span className="glyph-missing" key={i}>
            {run.text}
          </span>
        ) : (
          <span key={i}>{run.text}</span>
        ),
      )}
    </>
  );
}
