/* 把 mock 注入脚本塞进 dist/index.html，生成 dist/mock.html（仅用于本地截图预览）。 */
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const dist = fileURLToPath(new URL("../dist/", import.meta.url));
const prev = fileURLToPath(new URL("./", import.meta.url));
const html = readFileSync(dist + "index.html", "utf8");
const inject = readFileSync(prev + "mock-inject.js", "utf8");
const out = html.replace("<head>", "<head>\n    <script>\n" + inject + "\n    </script>");
writeFileSync(dist + "mock.html", out);

// vite build 会清空 dist，字体目录每次重建
const fonts = ["arial.ttf", "arialbd.ttf", "georgia.ttf", "georgiai.ttf", "times.ttf", "consola.ttf", "impact.ttf", "comic.ttf", "segoeui.ttf", "verdana.ttf"];
mkdirSync(dist + "mock-fonts", { recursive: true });
let copied = 0;
for (const f of fonts) {
  const src = "C:/Windows/Fonts/" + f;
  if (existsSync(src) && !existsSync(dist + "mock-fonts/" + f)) {
    copyFileSync(src, dist + "mock-fonts/" + f);
    copied++;
  }
}
console.log("wrote dist/mock.html, fonts:", copied);
