// The scenarios of livr/benches/validate.rs (and of dart-livr and
// js-validator-livr): keep them in sync
export const simpleSchema = {
  name: ['required', 'string'],
  email: ['required', 'email'],
  age: ['required', 'positive_integer'],
  gender: { one_of: ['male', 'female'] },
  password: ['required', { min_length: 8 }],
};

export const simpleInput = (i) => ({
  name: 'John Doe',
  email: `john.doe${i}@example.com`,
  age: i % 2 === 0 ? 25 : '25',
  gender: 'male',
  password: 'secret-password',
  extra: 'stripped',
});

export const invalidSimpleInput = (i) => ({
  name: '',
  email: `john.doe${i}@`,
  age: -1,
  gender: 'unknown',
  password: 'short',
});

export const nestedSchema = {
  id: ['required', 'positive_integer'],
  user: [
    'required',
    {
      nested_object: {
        name: ['required', 'string'],
        address: ['required', { nested_object: { city: ['required', 'string'], zip: ['required', { like: '^\\d{5}$' }] } }],
      },
    },
  ],
  tags: { list_of: ['required', 'string'] },
};

export const nestedInput = (i) => ({
  id: i + 1,
  user: { name: 'John Doe', address: { city: 'Kyiv', zip: '01001', extra: 'stripped' } },
  tags: ['rust', 'livr', 'fast'],
});

export const POOL = 1000;

// Best of 5 runs of 0.5 s after 0.5 s of warm-up, like the Dart benchmark
export function measure(fn) {
  const run = (ms) => {
    let ops = 0;
    const start = performance.now();
    let now = start;
    while (now - start < ms) {
      for (let k = 0; k < 1000; k++) fn(ops++ % POOL);
      now = performance.now();
    }
    return (ops / (now - start)) * 1000;
  };
  run(500);
  let best = 0;
  for (let r = 0; r < 5; r++) best = Math.max(best, run(500));
  return best;
}
