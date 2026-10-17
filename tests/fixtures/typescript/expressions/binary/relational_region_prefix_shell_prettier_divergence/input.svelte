<script lang="ts">
	// Between a sibling holding a `<` and a later sibling's `>` that may close the region, a pair
	// the author wrote directly behind `typeof` or a unary `-` stays
	fn(a < b, typeof (c) > (d, e));
	fn(a < b, typeof (c), d > (e, f));
	fn(a < b, -(1) > (d, e));
	fn(a < b, -(1.5), d > (e, f));
	// The carriers
	new Cls(a < b, typeof (c) > (d, e));
	fn?.(a < b, typeof (c) > (d, e));
	obj.fn(a < b, typeof (c) > (d, e));
	const a1 = [a < b, typeof (c) > (d, e)];
	(a < b, typeof (c) > (d, e));
	const a2 = `${(a < b, typeof (c) > (d, e))}`;
	for (a < b, typeof (c) > (d, e); ;) {}
	// What the pair holds, and where the operator stands in its sibling
	fn(a < b, typeof (c.d) > (e, f));
	fn(a < b, typeof (c)[d] > (e, f));
	fn(a < b, typeof (c).d > (e, f));
	fn(a < b, typeof (c.d).e > (f, g));
	fn(a < b, typeof (c)() > (d, e));
	fn(a < b, typeof (c)().d > (e, f));
	fn(a < b, typeof (c)`t` > (d, e));
	fn(a < b, typeof (c)! > (d, e));
	fn(a < b, typeof (this), d > (e, f));
	fn(a < b, (c) => typeof (c) > (d, e));
	fn(a < b, y, typeof (c), d > (e, f));
	// The `<` element's own tail is in the region
	fn(a < typeof (c), d > (e, f));
	fn(a < -(1), d > (e, f));
	// The follower is not read
	fn(a < b, typeof (c) > (d - e) * 2);
	fn(a < b, typeof (c) > `t`);
	fn(a < b, typeof (c) > d);
	// The region is the parser's flag, so a `<` side no list can be read from keeps the pair too
	fn(a < b + 1, typeof (c) > (d, e));
	// Where the bare spelling reads the same, both authorings stay
	fn(a < b, typeof (c) === 's', d > (e, f));
	fn(a < b, typeof c === 's', d > (e, f));
	fn(a < b, -(c), d > (e, f));
	fn(a < b, -c, d > (e, f));
	// Outside a region, or behind another operator, the pair strips
	fn(a < b, typeof c);
	fn(a < b, typeof c >= d);
	fn(a <= b, typeof c > (d, e));
	fn(a + b, typeof c > (d, e));
	fn(g(a < b), typeof c > (d, e));
	fn(typeof c, a < b + 1, d > (e, f));
	const b1 = { p: a < b, q: typeof c > (d, e) };
	fn(a < b, void c, d > (e, f));
	fn(a < b, !c > d);
	fn(a < b, +1, d > (e, f));
	fn(a < b, c + 1, d > (e, f));
	// A pair the operand needs stays anywhere
	fn(a < b, typeof (c + d) > (e, f));
	fn(a < b, -(-1), d > (e, f));
</script>

{fn(a < b, typeof (c) > (d, e))}
<div data-attr={fn(a < b, -(1) > (d, e))}></div>
{#if fn(a < b, typeof (c), d > (e, f))}<p>text</p>{/if}
