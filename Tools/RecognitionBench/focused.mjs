// Additional ablation: narrow text bands, block segmentation and word boxes.
import fs from 'node:fs/promises';
import path from 'node:path';
import sharp from 'sharp';
import { createWorker, PSM } from 'tesseract.js';
const [input, previous, output] = process.argv.slice(2);
if (!output) throw new Error('Usage: node focused.mjs INPUT_JSON OCR_JSON OUTPUT_JSON');
const items = JSON.parse(await fs.readFile(input));
const existing = JSON.parse(await fs.readFile(previous));
const worker = await createWorker('eng', 1, { cachePath: path.dirname(output) });
await worker.setParameters({ tessedit_pageseg_mode: PSM.SINGLE_BLOCK, preserve_interword_spaces: '1', user_defined_dpi: '300' });
const results = [];
try {
  for (const item of items) {
    const old = existing.find(x => x.id === item.id);
    const { width, height } = await sharp(item.path).metadata();
    const extra = [];
    const start = performance.now();
    for (const [x, y, w, h] of [[.12,.025,.73,.1], [0,.91,.52,.07]]) {
      const region = { left: Math.round(x*width), top: Math.round(y*height), width: Math.floor(w*width), height: Math.floor(h*height) };
      const { data: image, info } = await sharp(item.path).extract(region).resize({width: 1000}).grayscale().normalize().png().toBuffer({resolveWithObject:true});
      const { data } = await worker.recognize(image, {}, {blocks:true});
      const lines = (data.blocks ?? []).flatMap(b => b.paragraphs ?? []).flatMap(p => p.lines ?? []);
      for (const line of lines) for (const piece of [line,...(line.words ?? [])]) {
        const box = piece.bbox;
        extra.push({text:piece.text.trim(),confidence:piece.confidence/100,
          x:(region.left+box.x0/info.width*region.width)/width,
          y:1-(region.top+box.y1/info.height*region.height)/height,
          width:(box.x1-box.x0)/info.width*region.width/width,
          height:(box.y1-box.y0)/info.height*region.height/height});
      }
    }
    results.push({...old, focusedSeconds:(performance.now()-start)/1000, focusedLines:extra,
      variants:{...old.variants, focused:extra, ensemble:[...old.variants.combined,...extra]}});
    await fs.writeFile(output,JSON.stringify(results,null,2));
    console.log(item.id,extra.filter(x=>x.y>.8).map(x=>x.text).join(' | '));
  }
} finally { await worker.terminate(); }
