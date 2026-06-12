// JSDoc cast between identifier and = when inner expression has block comment
var a /** @type {number} */ = /** @type {string} */ b.c;

// Nested JSDoc casts with long call expression
const d /** @type {any[]} */ = /** @type {A} */ e.f.find(
	(x) => x.type === 'aaaa' && x.name === 'bbbbb' && x.value !== true,
);
