// JSDoc cast parens spanning multiple lines - no spurious blank line after
const a = /** @type {A} */ b.find(
	(x) => x.type === 'cccc' && x.name === 'ddddddddddddddddddddddddddddddddddddd',
);
const e = a && typeof a.value === 'object';

// Multiple consecutive multiline JSDoc casts
const f = /** @type {A} */ b.find(
	(x) => x.type === 'ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc',
);
const g = /** @type {B} */ b.find(
	(x) => x.name === 'ddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
);
const h = 1;
