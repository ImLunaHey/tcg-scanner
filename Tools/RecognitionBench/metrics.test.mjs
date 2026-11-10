import test from 'node:test';
import assert from 'node:assert/strict';
import { outcome, counts, median, retrieval } from './metrics.mjs';
test('ambiguity, wrong suggestions and absent results are not successes',()=>{
 assert.equal(outcome('a',['a','b']),'ambiguous');
 assert.equal(outcome('a',['b']),'wrong');
 assert.equal(outcome(null,['b']),'falsePositive');
 assert.equal(outcome(null,[]),'rejected');
 assert.throws(()=>counts([{id:'x',expected:'a'}],{}),/Missing/);
});
test('retrieval reports missing references separately and excludes backs',()=>{
 const items=[{id:'one',expected:'a'},{id:'two',expected:'b'},{id:'back',expected:null}];
 const results=items.map(x=>({id:x.id,ranked:[{id:'a'}]}));
 assert.deepEqual(retrieval(items,results,[{id:'a'}]),{covered:1,missingReference:1,top1:1,top5:1});
});
test('median handles even and odd samples without modifying them',()=>{
 const values=[3,1,2,4];assert.equal(median(values),2.5);assert.deepEqual(values,[3,1,2,4]);
 assert.equal(median([3,1,2]),2);assert.equal(median([]),null);
});
