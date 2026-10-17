<script lang="ts">
	// Between a sibling holding a `<` and a later sibling's `>` that may close the region, an
	// arrow function's conditional body and an assignment print without a pair of their own
	fn(a < b, (c) => (c ? d : e), f > (g, h));
	fn(a < b, (c = d), e > (f, g));
	// The carriers
	new Cls(a < b, (c) => (c ? d : e), f > (g, h));
	fn?.(a < b, (c) => (c ? d : e), f > (g, h));
	obj.fn(a < b, (c) => (c ? d : e), f > (g, h));
	const a1 = [a < b, (c) => (c ? d : e), f > (g, h)];
	(a < b, (c) => (c ? d : e), f > (g, h));
	(a < b, (c = d), e > (f, g));
	const a2 = `${(a < b, (c) => (c ? d : e), f > (g, h))}`;
	for (a < b, (c) => (c ? d : e), f > (g, h); ;) {}
	// The arrow's own shape: no parameter, a typed one, a curried arrow, the closer in its body
	fn(a < b, () => (c ? d : e), f > (g, h));
	fn(a < b, (c: T) => (c ? d : e), f > (g, h));
	fn(a < b, (c) => (d) => (d ? e : f), g > (h, i));
	fn(a < b, (c) => (c ? d : e > (g, h)));
	// The assignment's positions: an arrow body, an array element, a template substitution,
	// and a pattern target
	fn(a < b, (c) => (d = e), f > (g, h));
	fn(a < b, [(c = d)], e > (f, g));
	fn(a < b, `${(c = d)}`, e > (f, g));
	fn(a < b, ([c] = d), e > (f, g));
	fn(a < b, ({ c } = d), e > (f, g));
	// The shape stands anywhere between the two, deeper in its sibling too
	fn(a < b, y, (c) => (c ? d : e), f > (g, h));
	fn(a < b, (c) => (c ? d : e), f > (g, h), y);
	fn(a < b, [(c) => (c ? d : e)], f > (g, h));
	fn(a < b.c, (d) => (d ? e : f), g > (h, i));
	fn(x && a < b, (c) => (c ? d : e), f > (g, h));
	// The follower is not read, nor what the conditional or the assignment holds
	fn(a < b, (c) => (c ? d : e), f > (g - h) * 2);
	fn(a < b, (c) => (c ? d : e), f > `t`);
	fn(a < b, (c) => (c ? d : e), f > g);
	fn(a < b, (c) => (c ? 1 : 2), f > (g, h));
	fn(a < b, (c) => (c.d ? e : f), g > (h, i));
	fn(a < b, (c += d), e > (f, g));
	// The region is the parser's flag, so a `<` side no list can be read from and an authored
	// pair around the comparison withhold the pairs too
	fn(a < b + 1, (c) => (c ? d : e), f > (g, h));
	fn(a < b, (c) => (c ? d : e), f > (g, h));
	// The pairs stay where no later sibling holds a closer or no sibling opens a region
	fn(a < b, (c) => (c ? d : e));
	fn(a < b, (c) => (c ? d : e), f >= g);
	fn(a <= b, (c) => (c ? d : e), f > (g, h));
	fn(a + b, (c = d), e > (f, g));
	fn((c) => (c ? d : e), a < b + 1, f > (g, h));
	fn(g(a < b), (c) => (c ? d : e), f > (g, h));
	const b1 = { p: a < b, q: (c) => (c ? d : e), r: f > (g, h) };
	// A pair the tree needs stays in the region
	fn(a < b, (c = d) + 1, e > (f, g));
	fn(a < b, (c = d) ? e : f, g > (h, i));
	// Neither shape takes a pair at all here
	fn(a < b, c ? d : e, f > (g, h));
	fn(a < b, (c) => c + 1, f > (g, h));
	for (a < b, c = d, e > (f, g); ;) {}
</script>

{fn(a < b, (c) => (c ? d : e), f > (g, h))}
<div data-attr={fn(a < b, (c = d), e > (f, g))}></div>
{#if fn(a < b, (c) => (c ? d : e), f > (g, h))}<p>text</p>{/if}
