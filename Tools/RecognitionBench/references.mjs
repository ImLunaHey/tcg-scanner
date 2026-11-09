import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import sharp from 'sharp';
const [input, output] = process.argv.slice(2);
if (!output) throw new Error('Usage: node references.mjs INPUT_JSON OUTPUT_DIRECTORY');
await fs.mkdir(path.join(output, 'references'), { recursive: true });
async function get(url) {
  for (let attempt = 0; attempt < 3; attempt++) {
    const response = await fetch(url, { redirect: 'error', signal: AbortSignal.timeout(20000) });
    if (response.ok) return Buffer.from(await response.arrayBuffer());
    if (![429, 500, 502, 503, 504].includes(response.status) || attempt === 2) throw new Error(`${response.status} ${url}`);
    await new Promise(resolve => setTimeout(resolve, 500 * (attempt + 1)));
  }
}
const items = JSON.parse(await fs.readFile(input));
const catalogueBytes = await get('https://api.tcgdex.net/v2/en/cards');
await fs.writeFile(path.join(output, 'catalogue.json'), catalogueBytes);
const catalogue = JSON.parse(catalogueBytes).filter(c => !c.image?.includes('/tcgp/'));
// Keep the four formerly uncatalogued Delta Reign queries out of corpus
// construction even after their labels are corrected. They test missing references.
const indexedItems = items.filter(i => !i.id.startsWith('gap-'));
const expected = new Set(indexedItems.map(i => i.expected).filter(Boolean));
const names = new Set(indexedItems.filter(i => i.expected).map(i => i.name.toLowerCase()));
const chosen = new Map(catalogue.filter(c => expected.has(c.id)).map(c => [c.id, c]));
// A labelled retrieval corpus: positive reference images plus up to four other
// printings per name, pooled together. No labels are passed to image inference.
for (const name of [...names].sort()) {
  for (const card of catalogue.filter(c => c.name.toLowerCase() === name && c.image && !expected.has(c.id)).slice(0, 4)) chosen.set(card.id, card);
}
const references = [], failures = [];
for (const card of chosen.values()) {
  try {
    if (!card.image) throw new Error('No catalogue image');
    const url = new URL(card.image + '/high.png');
    if (url.protocol !== 'https:' || url.hostname !== 'assets.tcgdex.net') throw new Error('Unexpected asset host');
    const destination = path.resolve(output, 'references', card.id + '.png');
    let bytes;
    try { bytes = await fs.readFile(destination); if (!bytes.length) throw new Error('Empty cache'); } catch { bytes = await get(url); }
    if (!bytes.length || bytes.length > 8 * 1024 * 1024) throw new Error('Empty or oversized reference');
    const info = await sharp(bytes).metadata();
    if (!info.width || !info.height || info.width * info.height > 16000000) throw new Error('Invalid reference dimensions');
    await fs.writeFile(destination, bytes);
    references.push({ id: card.id, name: card.name, path: destination, url: String(url), sha256: crypto.createHash('sha256').update(bytes).digest('hex') });
    console.log(card.id);
  } catch (error) { failures.push({ id: card.id, error: String(error) }); }
  await fs.writeFile(path.join(output, 'references.json'), JSON.stringify({ references, failures }, null, 2));
}
