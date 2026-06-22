// Short: JSDoc cast inline with chain call arg after paren stripping
set = a.b(c.d([e.f('g')], /** @type {T} */ h.visit(i.k('=', /** @type {U} */ j.m, e.f('g')))));

// Exactly 100 chars: JSDoc cast inline, call fits on one line
a.b(
	[c],
	/** @type {AAAA} */ d.eee(ffffffffffffffffffffffffffffffffffffffffffff, gggggggggggggggggggggggg)
);

// 101 chars if inline: JSDoc cast inline, inner call expands
a.b(
	[c],
	/** @type {AAAA} */ d.eee(fffffffffffffffffffffffffffffffffffffffffffff, gggggggggggggggggggggggg)
);
