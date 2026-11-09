export function outcome(expected, candidates) {
  if (expected === null) return candidates.length ? 'falsePositive' : 'rejected';
  if (!candidates.length) return 'unresolved';
  if (candidates.length > 1) return 'ambiguous';
  return candidates[0] === expected ? 'correct' : 'wrong';
}
export function counts(items, predictions) {
  const result = { correct:0, wrong:0, unresolved:0, ambiguous:0, rejected:0, falsePositive:0 };
  for (const item of items) {
    if (!(item.id in predictions)) throw new Error(`Missing result ${item.id}`);
    result[outcome(item.expected, predictions[item.id])]++;
  }
  return result;
}
export function median(values) {
  if (!values.length) return null;
  const sorted = [...values].sort((a,b) => a-b), mid = Math.floor(sorted.length/2);
  return sorted.length%2 ? sorted[mid] : (sorted[mid-1]+sorted[mid])/2;
}
export function retrieval(items, results, references) {
  const available = new Set(references.map(x => x.id));
  const covered = items.filter(x => x.expected && available.has(x.expected));
  const predictions = new Map(results.map(x => [x.id,x.ranked]));
  return { covered:covered.length, missingReference:items.filter(x=>x.expected&&!available.has(x.expected)).length,
    top1:covered.filter(x=>predictions.get(x.id)?.[0]?.id===x.expected).length,
    top5:covered.filter(x=>predictions.get(x.id)?.slice(0,5).some(p=>p.id===x.expected)).length };
}
