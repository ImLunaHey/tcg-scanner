import fs from 'node:fs/promises';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { pipeline, env } from '@huggingface/transformers';
const [input, referencesFile, output] = process.argv.slice(2);
if (!output) throw new Error('Usage: node embeddings.mjs INPUT_JSON REFERENCES_JSON OUTPUT_JSON');
const model = 'onnx-community/dinov3-vits16-pretrain-lvd1689m-ONNX';
const revision = '48988dfe73065df8d6f5ccc0edc7c8bcf307de41';
env.cacheDir = path.resolve(path.dirname(output), 'models');
const items = JSON.parse(await fs.readFile(input));
const { references } = JSON.parse(await fs.readFile(referencesFile));
const started = performance.now();
const extractor = await pipeline('image-feature-extraction', model, { revision, dtype: 'fp32', device: 'cpu' });
const startupSeconds = (performance.now() - started) / 1000;
async function embedding(file) {
  const output = await extractor(file);
  // DINOv3 first token is CLS; four register tokens follow. Do not average them
  // with image patches. Cosine retrieval uses a normalized CLS descriptor.
  const dimension = output.dims.at(-1);
  const vector = Array.from(output.data.slice(0, dimension));
  const norm = Math.hypot(...vector);
  return vector.map(v => v / norm);
}
const index = [];
const cacheFile = path.join(path.dirname(output), 'reference-embeddings.json');
let cached = {};
try { cached = JSON.parse(await fs.readFile(cacheFile)); } catch {}
for (const reference of references) {
  const key = `${revision}:${reference.sha256}`;
  const vector = cached[key] ?? await embedding(reference.path);
  cached[key] = vector;
  index.push({ id: reference.id, vector });
  await fs.writeFile(cacheFile, JSON.stringify(cached));
  console.log('REFERENCE', reference.id);
}
const results = [];
for (const item of items) {
  const start = performance.now();
  const vector = await embedding(item.path);
  const ranked = index.map(ref => ({ id: ref.id, similarity: ref.vector.reduce((sum, v, i) => sum + v * vector[i], 0) }))
    .sort((a,b) => b.similarity - a.similarity);
  results.push({ id: item.id, seconds: (performance.now() - start) / 1000, ranked: ranked.slice(0, 5) });
  await fs.writeFile(output, JSON.stringify({ model, revision, dtype: 'fp32', descriptor: 'normalized CLS', startupSeconds, referenceCount: index.length, results }, null, 2));
  console.log('QUERY', item.id, ranked[0]);
}
await extractor.dispose();
