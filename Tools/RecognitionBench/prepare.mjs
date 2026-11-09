// Read existing manifests only. Copy/resize fixtures into an ignored experiment directory.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import sharp from 'sharp';
const [current, archive, output, labelsFile] = process.argv.slice(2);
if (!output) throw new Error('Usage: node prepare.mjs CURRENT_BATCH ARCHIVE_ROOT OUTPUT [LABEL_OVERRIDES_JSON]');
await fs.mkdir(path.join(output, 'images'), { recursive: true });
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const items = [];
async function batch(root, tag, filter, labelSource) {
  const record = JSON.parse(await fs.readFile(path.join(root, 'manifest.json')));
  for (const identification of record.cardIdentifications ?? []) {
    const sequence = identification.request.sequence;
    if (!filter(identification)) continue;
    const original = record.images.find(x => x.sequence === sequence);
    const edit = record.processedImages.findLast(x => x.request.sequence === sequence && x.cardPreparation && x.sourceSHA256 === original.sha256);
    if (!edit) throw new Error(`Missing prepared image ${tag}/${sequence}`);
    const source = path.join(root, edit.image.relativePath);
    const bytes = await fs.readFile(source);
    if (hash(bytes) !== edit.image.sha256) throw new Error(`Checksum mismatch ${source}`);
    const id = `${tag}-${String(sequence).padStart(3, '0')}`;
    const normalized = await sharp(bytes).resize({ width: 840 }).png().toBuffer();
    const destination = path.resolve(output, 'images', id + '.png');
    await fs.writeFile(destination, normalized);
    const candidates = identification.evidence?.candidates ?? [];
    items.push({ id, path: destination, sha256: hash(normalized), source, sourceSHA256: hash(bytes),
      expected: candidates.length === 1 ? candidates[0].id : null,
      name: candidates[0]?.name ?? identification.evidence?.uncataloguedCard?.name ?? 'Back',
      labelSource, kind: candidates.length === 1 ? 'positive' : identification.evidence?.uncataloguedCard ? 'catalogue-gap' : 'back' });
  }
}
await batch(current, 'current', i => i.evidence?.candidates?.length === 1, 'saved suggestion; requires independent inspection');
await batch(path.join(archive, 'B-010'), 'regression', i => i.evidence?.candidates?.length === 1, 'existing eleven-card regression');
await batch(path.join(archive, 'B-049'), 'hard', i => i.request.sequence === 46, 'existing Binacle regression');
await batch(path.join(archive, 'B-051'), 'gap', i => [2, 4, 6, 8].includes(i.request.sequence), 'printed details; catalogue gap at capture');
await batch(current, 'negative', i => [3, 5, 11, 15, 19, 21].includes(i.request.sequence), 'prepared standard back');
if (labelsFile) {
  const overrides = JSON.parse(await fs.readFile(labelsFile));
  for (const item of items) if (overrides[item.id]) Object.assign(item, overrides[item.id]);
}
items.sort((a,b) => a.id.localeCompare(b.id));
await fs.writeFile(path.join(output, 'input.json'), JSON.stringify(items, null, 2));
// Show title + footer at useful resolution for manual label inspection.
for (let offset = 0; offset < items.length; offset += 12) {
  const group = items.slice(offset, offset + 12);
  const tiles = [];
  for (let index = 0; index < group.length; index++) {
    const item = group[index];
    const thumb = await sharp(item.path).resize({ width: 240 }).png().toBuffer();
    tiles.push({ input: thumb, left: (index % 4) * 250, top: Math.floor(index / 4) * 380 + 28 });
    const label = `${item.id} ${item.expected ?? item.kind}`;
    tiles.push({ input: Buffer.from(`<svg width="250" height="28"><rect width="250" height="28" fill="white"/><text x="3" y="19" font-size="12">${label}</text></svg>`), left: (index % 4) * 250, top: Math.floor(index / 4) * 380 });
  }
  await sharp({ create: { width: 1000, height: 1140, channels: 3, background: 'white' }}).composite(tiles).png().toFile(path.join(output, `contact-${offset}.png`));
}
console.log(`Prepared ${items.length} checksum-verified fixtures`);
