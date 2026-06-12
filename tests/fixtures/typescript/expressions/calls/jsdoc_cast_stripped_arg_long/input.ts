// Short: JSDoc cast in call arg - fits on one line
a(b, /** @type {T} */ fn(c, d, e));

// Multi-arg boundary 100: fits at exactly 100 chars
a(b______, /** @type {Expression} */ fn(c, d, e__________________________________________________));

// Multi-arg boundary 101: breaks
a(
	b_______,
	/** @type {Expression} */ fn(c, d, e__________________________________________________),
);

// Expanded with comment inline: comment stays on same line as expression
a(
	b________________________________________,
	/** @type {Expression} */ fn(c, d, e__________________________________________________),
);
