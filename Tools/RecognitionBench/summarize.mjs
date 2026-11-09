import fs from 'node:fs/promises';
import path from 'node:path';
import { counts, median, retrieval } from './metrics.mjs';
const [root] = process.argv.slice(2);
const read = async name => JSON.parse(await fs.readFile(path.join(root,name)));
const items=await read('input.json'), native=await read('native.json'), ocr=await read('focused.json'), matches=await read('tesseract-matches.json'), dino=await read('dino.json'), refs=await read('references.json');
if ([native,ocr,matches,dino.results].some(x=>x.length!==items.length)) throw new Error('Incomplete experiment');
const summary={images:items.length,fronts:items.filter(x=>x.expected).length,backs:items.filter(x=>!x.expected).length,uniquePrintings:new Set(items.map(x=>x.expected).filter(Boolean)).size,
 native:{...counts(items,Object.fromEntries(native.map(x=>[x.id,x.evidence.candidates.map(c=>c.id)]))),medianSeconds:median(native.map(x=>x.seconds))},tesseract:{},dino:{...retrieval(items,dino.results,refs.references),referenceCount:refs.references.length,medianSeconds:median(dino.results.map(x=>x.seconds))}};
for(const key of Object.keys(matches[0].variants)){
 const durations=ocr.map(x=> key==='whole'?x.seconds.whole:key==='regional'?x.seconds.title+x.seconds.footer+x.seconds.left+x.seconds.right:key==='focused'?x.focusedSeconds:Object.values(x.seconds).reduce((a,b)=>a+b,0)+(key==='ensemble'?x.focusedSeconds:0));
 summary.tesseract[key]={...counts(items,Object.fromEntries(matches.map(x=>[x.id,x.variants[key]]))),medianOCRSeconds:median(durations)};
}
summary.currentBatch = { images:items.filter(x=>x.id.startsWith('current-')).length };
for (const key of ['native', ...Object.keys(matches[0].variants)]) {
 const predictions=key==='native'?Object.fromEntries(native.map(x=>[x.id,x.evidence.candidates.map(c=>c.id)])):Object.fromEntries(matches.map(x=>[x.id,x.variants[key]]));
 summary.currentBatch[key]=counts(items.filter(x=>x.id.startsWith('current-')),predictions);
}
// Simulate a missing correct reference without rerunning inference: other
// indexed printings remain. These are retrieval scores, never acceptance probabilities.
const excluded=items.filter(x=>x.expected&&refs.references.some(r=>r.id===x.expected)).map(item=>{
 const row=dino.results.find(x=>x.id===item.id);const wrong=row.ranked.find(x=>x.id!==item.expected);
 return {id:item.id,wrong};
});
summary.dino.withheldCorrectReference={maximumWrongSimilarity:Math.max(...excluded.map(x=>x.wrong.similarity)),examples:excluded.sort((a,b)=>b.wrong.similarity-a.wrong.similarity).slice(0,3)};
await fs.writeFile(path.join(root,'summary.json'),JSON.stringify(summary,null,2));
console.log(JSON.stringify(summary,null,2));
