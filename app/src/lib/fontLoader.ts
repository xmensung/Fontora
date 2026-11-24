

import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import type { FontFace as ZFontFace } from "./ipc";

type LoadState = "loading" | "loaded" | "failed";
const cache = new Map<string, LoadState>();
const listeners = new Map<string, Set<() => void>>();

function cssName(face: ZFontFace): string {

  return `zfm-${hash(face.id)}`;
}

function hash(s: string): string {
  let h = 5381;
  for (let i = 0; i < s.length; i++) h = ((h << 5) + h + s.charCodeAt(i)) >>> 0;
  return h.toString(36);
}

function ensureLoaded(face: ZFontFace): void {
  ensurePathLoaded(cssName(face), face.previewPath ?? face.path);
}

function ensurePathLoaded(name: string, path: string): void {
  if (cache.has(name)) return;
  cache.set(name, "loading");

  const url = convertFileSrc(path);
  const ff = new FontFace(name, `url("${url}")`);
  ff.load()
    .then(() => {
      document.fonts.add(ff);
      cache.set(name, "loaded");
    })
    .catch(() => {
      cache.set(name, "failed");
    })
    .finally(() => {
      listeners.get(name)?.forEach((fn) => fn());
      listeners.delete(name);
    });
}

export function useFontCss(face: ZFontFace | null): {
  fontFamily: string | null;
  failed: boolean;
} {
  const name = face ? cssName(face) : null;
  const [, bump] = useState(0);

  useEffect(() => {
    if (!face || !name) return;
    ensureLoaded(face);
    if (cache.get(name) === "loading") {
      const set = listeners.get(name) ?? new Set();
      const fn = () => bump((n) => n + 1);
      set.add(fn);
      listeners.set(name, set);
      return () => {
        set.delete(fn);
      };
    }
  }, [face, name]);

  const state = name ? cache.get(name) : undefined;
  return {
    fontFamily: state === "loaded" && name ? name : null,
    failed: state === "failed",
  };
}

export function usePathFontCss(key: string | null, path: string | null): string | null {
  const name = key ? `zfm-p-${hash(key)}` : null;
  const [, bump] = useState(0);

  useEffect(() => {
    if (!name || !path) return;
    ensurePathLoaded(name, path);
    if (cache.get(name) === "loading") {
      const set = listeners.get(name) ?? new Set();
      const fn = () => bump((n) => n + 1);
      set.add(fn);
      listeners.set(name, set);
      return () => {
        set.delete(fn);
      };
    }
  }, [name, path]);

  return name && path && cache.get(name) === "loaded" ? name : null;
}

export function loadFaceCss(face: ZFontFace): Promise<string> {
  const name = cssName(face);
  ensureLoaded(face);
  const state = cache.get(name);
  if (state === "loaded") return Promise.resolve(name);
  if (state === "failed") return Promise.reject(new Error("font failed to load"));
  return new Promise((resolve, reject) => {
    const set = listeners.get(name) ?? new Set();
    set.add(() => {
      if (cache.get(name) === "loaded") resolve(name);
      else reject(new Error("font failed to load"));
    });
    listeners.set(name, set);
  });
}

/**
 * 真的有墨吗。
 *
 * 有一类日文毛笔字体把 ASCII 码位都挂上了「空字形」：cmap 查得到、字体也加载
 * 成功，可画出来什么都没有 —— 界面上就是一张空白卡片，连报错都没有。
 * 光看覆盖率查不出这种情况，只能真画一遍数像素。
 */
const inkCache = new Map<string, boolean>();
let inkCtx: CanvasRenderingContext2D | null = null;

function inkCanvas(): CanvasRenderingContext2D | null {
  if (inkCtx) return inkCtx;
  const canvas = document.createElement("canvas");
  canvas.width = 256;
  canvas.height = 72;
  inkCtx = canvas.getContext("2d", { willReadFrequently: true });
  return inkCtx;
}

export function sampleHasInk(fontFamily: string, text: string): boolean {
  const sample = Array.from(text).slice(0, 60).join("");
  if (!sample.trim()) return true;
  const key = `${fontFamily}\u0000${sample}`;
  const hit = inkCache.get(key);
  if (hit !== undefined) return hit;

  const ctx = inkCanvas();
  if (!ctx) return true;
  ctx.font = `40px "${fontFamily}"`;
  // 画布只铺这么宽，读回来的像素少一个数量级
  const width = Math.min(1024, Math.max(64, Math.ceil(ctx.measureText(sample).width) + 8));
  if (ctx.canvas.width !== width) ctx.canvas.width = width;
  const height = ctx.canvas.height;

  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, width, height);
  ctx.font = `40px "${fontFamily}"`;
  ctx.fillStyle = "#000";
  ctx.textBaseline = "alphabetic";
  ctx.fillText(sample, 2, 56);
  const pixels = ctx.getImageData(0, 0, width, height).data;
  let ink = false;
  for (let i = 3; i < pixels.length; i += 4) {
    if (pixels[i] > 8) {
      ink = true;
      break;
    }
  }
  inkCache.set(key, ink);
  return ink;
}

/** 这段文字用这款字面画出来是不是有东西。字体还没加载好时按「有」处理，别闪。 */
export function useInkOk(fontFamily: string | null, text: string): boolean {
  const [ok, setOk] = useState(true);
  useEffect(() => {
    if (!fontFamily || !text.trim()) {
      setOk(true);
      return;
    }
    setOk(sampleHasInk(fontFamily, text));
  }, [fontFamily, text]);
  return ok;
}

/**
 * 逐字找「空字形」。
 *
 * 整句检测只能回答「这一行有没有墨」，可有些字体是一半正常、一半空 ——
 * 有墨的字用毛笔体（笔画粗），空的字掉成一片空白，读起来就是「糊成一坨、
 * 粗细极不均匀」。所以把每个字单独画进一个格子，逐格数像素，把空的挑出来。
 */
const blankCache = new Map<string, Set<number>>();
let gapCtx: CanvasRenderingContext2D | null = null;

function gapCanvas(): CanvasRenderingContext2D | null {
  if (gapCtx) return gapCtx;
  const canvas = document.createElement("canvas");
  gapCtx = canvas.getContext("2d", { willReadFrequently: true });
  return gapCtx;
}

export function sampleBlankGaps(fontFamily: string, text: string): Set<number> {
  const chars = Array.from(new Set(Array.from(text))).filter((c) => c.trim());
  if (!chars.length) return new Set();

  const key = `${fontFamily}\u0000${chars.join("")}`;
  const hit = blankCache.get(key);
  if (hit) return hit;

  const ctx = gapCanvas();
  if (!ctx) return new Set();

  const CELL = 56;
  const HEIGHT = 64;
  const width = CELL * chars.length;
  // 画布有宽度上限；超长文本不值得为它逐字验一遍，交给整句检测兜底。
  if (width > 4096) return new Set();
  if (ctx.canvas.width !== width) ctx.canvas.width = width;
  if (ctx.canvas.height !== HEIGHT) ctx.canvas.height = HEIGHT;

  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, width, HEIGHT);
  ctx.font = `44px "${fontFamily}"`;
  ctx.fillStyle = "#000";
  ctx.textBaseline = "alphabetic";
  chars.forEach((c, i) => {
    ctx.fillText(c, i * CELL + 6, 48);
  });

  const pixels = ctx.getImageData(0, 0, width, HEIGHT).data;
  const blank = new Set<number>();
  chars.forEach((c, i) => {
    let ink = false;
    const from = i * CELL;
    const to = Math.min(width, from + CELL);
    for (let y = 0; y < HEIGHT && !ink; y++) {
      const row = y * width;
      for (let x = from; x < to; x++) {
        if (pixels[(row + x) * 4 + 3] > 8) {
          ink = true;
          break;
        }
      }
    }
    if (!ink) blank.add(c.codePointAt(0) ?? 0);
  });

  blankCache.set(key, blank);
  return blank;
}

/** 逐字空字形集合；字体没加载好时返回空集，别闪。 */
export function useBlankGaps(fontFamily: string | null, text: string): Set<number> {
  const [gaps, setGaps] = useState<Set<number>>(() => new Set());
  useEffect(() => {
    if (!fontFamily || !text.trim()) {
      setGaps(new Set());
      return;
    }
    setGaps(sampleBlankGaps(fontFamily, text));
  }, [fontFamily, text]);
  return gaps;
}

export function pathBasename(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
