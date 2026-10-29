// js-validator-livr (and livr-wasm, when built) on the same scenarios as
// the Rust benchmark. Run pinned to one core: taskset -c 3 node bench.mjs
import { createRequire } from 'node:module';
import os from 'node:os';
import * as s from './scenarios.mjs';

const require = createRequire(import.meta.url);
const LIVR = require(process.env.LIVR_JS ?? '../../js-validator-livr/lib/LIVR.js');

const simpleInputs = Array.from({ length: s.POOL }, (_, i) => s.simpleInput(i));
const nestedInputs = Array.from({ length: s.POOL }, (_, i) => s.nestedInput(i));
const invalidInputs = Array.from({ length: s.POOL }, (_, i) => s.invalidSimpleInput(i));
const simpleJson = simpleInputs.map((x) => JSON.stringify(x));
const nestedJson = nestedInputs.map((x) => JSON.stringify(x));

const results = {};
const add = (impl, name, fn) => {
  (results[name] ??= {})[impl] = s.measure(fn);
};

const simpleValidator = new LIVR.Validator(s.simpleSchema).prepare();
const nestedValidator = new LIVR.Validator(s.nestedSchema).prepare();
add('js', 'simple, static', (i) => simpleValidator.validate(simpleInputs[i]));
add('js', 'simple, dynamic', (i) => new LIVR.Validator(s.simpleSchema).validate(simpleInputs[i]));
add('js', 'nested, static', (i) => nestedValidator.validate(nestedInputs[i]));
add('js', 'nested, dynamic', (i) => new LIVR.Validator(s.nestedSchema).validate(nestedInputs[i]));
add('js', 'invalid, static', (i) => simpleValidator.validate(invalidInputs[i]));
add('js', 'simple, static, JSON string', (i) => simpleValidator.validate(JSON.parse(simpleJson[i])));
add('js', 'nested, static, JSON string', (i) => nestedValidator.validate(JSON.parse(nestedJson[i])));

let wasm = null;
try {
  wasm = require('../livr-wasm/pkg/livr_wasm.js');
} catch {
  console.error('livr-wasm not built: wasm-pack build livr-wasm --target nodejs --release');
}
if (wasm) {
  const simpleW = new wasm.Validator(s.simpleSchema);
  const nestedW = new wasm.Validator(s.nestedSchema);
  // The outputs must match the ones of JS livr
  for (let i = 0; i < 10; i++) {
    const check = (a, b) => {
      if (JSON.stringify(a) !== JSON.stringify(b)) throw new Error(`output mismatch: ${JSON.stringify(a)} vs ${JSON.stringify(b)}`);
    };
    const sortKeys = (v) => JSON.parse(JSON.stringify(v, (_, x) => (x && typeof x === 'object' && !Array.isArray(x) ? Object.fromEntries(Object.entries(x).sort()) : x)));
    check(sortKeys(simpleW.validate(simpleInputs[i]).output), sortKeys(simpleValidator.validate(simpleInputs[i])));
    check(sortKeys(nestedW.validate(nestedInputs[i]).output), sortKeys(nestedValidator.validate(nestedInputs[i])));
    check(sortKeys(simpleW.validate(invalidInputs[i]).errors), sortKeys((simpleValidator.validate(invalidInputs[i]), simpleValidator.getErrors())));
  }
  // Objects as JSON strings both ways: JSON.stringify and JSON.parse of V8
  // are faster than reading objects property by property from WebAssembly
  const viaJson = (validator, data) => JSON.parse(validator.validateJsonToJson(JSON.stringify(data)));
  add('wasm, JSON both ways', 'simple, static', (i) => viaJson(simpleW, simpleInputs[i]));
  add('wasm, JSON both ways', 'nested, static', (i) => viaJson(nestedW, nestedInputs[i]));
  add('wasm, JSON both ways', 'invalid, static', (i) => viaJson(simpleW, invalidInputs[i]));
  const simpleSchemaJson = JSON.stringify(s.simpleSchema);
  const nestedSchemaJson = JSON.stringify(s.nestedSchema);
  add('wasm, JSON both ways', 'simple, dynamic', (i) => viaJson(wasm.Validator.fromJson(simpleSchemaJson), simpleInputs[i]));
  add('wasm, JSON both ways', 'nested, dynamic', (i) => viaJson(wasm.Validator.fromJson(nestedSchemaJson), nestedInputs[i]));
  add('wasm, JSON both ways', 'simple, static, JSON string', (i) => JSON.parse(simpleW.validateJsonToJson(simpleJson[i])));
  add('wasm, JSON both ways', 'nested, static, JSON string', (i) => JSON.parse(nestedW.validateJsonToJson(nestedJson[i])));
  add('wasm', 'simple, static', (i) => simpleW.validate(simpleInputs[i]));
  add('wasm', 'simple, dynamic', (i) => new wasm.Validator(s.simpleSchema).validate(simpleInputs[i]));
  add('wasm', 'nested, static', (i) => nestedW.validate(nestedInputs[i]));
  add('wasm', 'nested, dynamic', (i) => new wasm.Validator(s.nestedSchema).validate(nestedInputs[i]));
  add('wasm', 'invalid, static', (i) => simpleW.validate(invalidInputs[i]));
  add('wasm', 'simple, static, JSON string', (i) => simpleW.validateJson(simpleJson[i]));
  add('wasm', 'nested, static, JSON string', (i) => nestedW.validateJson(nestedJson[i]));
}

const fmt = (ops) => (ops === undefined ? '-' : ops >= 1e6 ? `${(ops / 1e6).toFixed(2)} M/s` : `${(ops / 1e3).toFixed(0)} K/s`);
console.log(`node ${process.version}, ${os.cpus()[0].model}, load ${os.loadavg().map((x) => x.toFixed(2)).join(' ')}`);
const impls = [...new Set(Object.values(results).flatMap((r) => Object.keys(r)))];
console.log(`| Scenario | ${impls.join(' | ')} |\n|---|${impls.map(() => '---').join('|')}|`);
for (const [name, r] of Object.entries(results)) console.log(`| ${name} | ${impls.map((impl) => `${fmt(r[impl])}`).join(' | ')} |`);
