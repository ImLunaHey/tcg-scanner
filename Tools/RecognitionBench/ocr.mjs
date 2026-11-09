import fs from 'node:fs/promises';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { createWorker, PSM } from 'tesseract.js';
import sharp from 'sharp';
const [input, output] = process.argv.slice(2);
if (!output) throw new Error('Usage: node ocr.mjs INPUT_JSON OUTPUT_JSON');
const items = JSON.parse(await fs.readFile(input));
const start = performance.now();
const worker = await createWorker('eng', 1, { cachePath: path.dirname(output) });
await worker.setParameters({ tessedit_pageseg_mode: PSM.SPARSE_TEXT, preserve_interword_spaces: '1', user_defined_dpi: '300' });
const startupSeconds = (performance.now() - start) / 1000;
const results = [];
try {
  for (const item of items.slice(0, Number(process.env.BENCH_LIMIT) || items.length)) {
    const { width, height } = await sharp(item.path).metadata();
    const regions = [
      ['whole', 0, 0, 1, 1], ['title', 0, 0, 1, .24],
      ['footer', 0, .68, 1, .32], ['left', 0, .82, .5, .18], ['right', .5, .82, .5, .18],
    ];
    const readings = {}, seconds = {};
    for (const [name, x, y, w, h] of regions) {
      const region = { left: Math.round(x * width), top: Math.round(y * height), width: Math.floor(w * width), height: Math.floor(h * height) };
      let image = sharp(item.path).extract(region);
      if (name !== 'whole') image = image.resize({ width: 820 }).grayscale().linear(1.28, 4);
      const { data: pixels, info } = await image.png().toBuffer({ resolveWithObject: true });
      const before = performance.now();
      const { data } = await worker.recognize(pixels, {}, { blocks: true, text: true });
      seconds[name] = (performance.now() - before) / 1000;
      readings[name] = (data.blocks ?? []).flatMap(b => b.paragraphs ?? []).flatMap(p => p.lines ?? []).map(line => {
        const box = line.bbox;
        return { text: line.text.trim(), confidence: line.confidence / 100,
          x: (region.left + box.x0 / info.width * region.width) / width,
          y: 1 - (region.top + box.y1 / info.height * region.height) / height,
          width: (box.x1 - box.x0) / info.width * region.width / width,
          height: (box.y1 - box.y0) / info.height * region.height / height };
      });
    }
    const regional = ['title','footer','left','right'].flatMap(key => readings[key]);
    results.push({ id: item.id, startupSeconds, seconds, readings,
      variants: { whole: readings.whole, regional, combined: [...readings.whole, ...regional] } });
    await fs.writeFile(output + '.tmp', JSON.stringify(results, null, 2));
    await fs.rename(output + '.tmp', output);
    console.log(item.id, Object.values(seconds).reduce((a,b) => a+b, 0).toFixed(2), readings.title.map(l => l.text).join(' | '));
  }
} finally { await worker.terminate(); }
