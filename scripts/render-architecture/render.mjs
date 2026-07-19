// clusters/pi/architecture/*.svg を puppeteer（同梱 Chromium）で 2x PNG にレンダリングする。
// SVG を編集したら `npm run render` で PNG を再生成し、両方をコミットする。
// フォントはホスト OS のものを使うため、PNG は macOS 上で生成する前提。
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { launch } from "puppeteer";

const dir = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../clusters/pi/architecture",
);
const svgs = (await readdir(dir)).filter((f) => f.endsWith(".svg")).sort();
if (svgs.length === 0) throw new Error(`no SVG found in ${dir}`);

const browser = await launch();
try {
  const page = await browser.newPage();
  for (const name of svgs) {
    const file = path.join(dir, name);
    const m = (await readFile(file, "utf8")).match(/viewBox="0 0 (\d+) (\d+)"/);
    if (!m) throw new Error(`viewBox not found: ${name}`);
    const width = Number(m[1]);
    const height = Number(m[2]);
    await page.setViewport({ width, height, deviceScaleFactor: 2 });
    await page.goto(pathToFileURL(file).href);
    const png = file.replace(/\.svg$/, ".png");
    await page.screenshot({ path: png });
    console.log(`render ${name} (${width}x${height}) -> ${path.basename(png)}`);
  }
} finally {
  await browser.close();
}
