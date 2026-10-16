<script lang="ts">
	// Authored pairs in comma lists
	fn((a < b), c > (d, e));
	new Cls((a < b), c > (d, e));
	fn?.((a < b), c > (d, e));
	obj.fn((a < b), c > (d, e));
	const a1 = [(a < b), c > (d, e)];
	((a < b), c > (d, e));
	for ((a < b), c > (d, e); ;) {}
	const a2 = `${((a < b), c > (d, e))}`;
	import((a < b), c > (d, e));
	// The paired element and closer can stand anywhere in the list
	fn(x, (a < b), c > (d, e));
	fn((a < b), y, c > (d, e));
	fn((a < b), c > (d, e), y);
	fn((a < b), (c < d), e > (f, g));
	// The comparison can stand anywhere inside the paired element; a `>>` closes two regions
	fn((a < b.c), d > (e, f));
	fn((a < 1), b > (c, d));
	fn((a.b < c), d > (e, f));
	fn((x && a < b), c > (d, e));
	fn((a < b < c), d >> (e, f));
	// The closer's follower does not decide whether the pair stays
	fn((a < b), c > (d - e) * 2);
	fn((a < b), c > `t`);
	fn((a < b), c > (await d));
	fn((a < b), c > await d);
	// A later sibling carries its closer through expression children
	fn((a < b), c > (d, e) && y);
	fn((a < b), (c) => c > (d - e) * 2);
	// Both spellings stay where the bare text still reads as comparisons
	fn((a < b), c > d);
	fn(a < b, c > d);
	// Redundant pairs strip where no later sibling holds a closer or the element holds no `<`
	fn(a < b, c);
	fn(a < b, c >= d);
	fn(a < b, () => c);
	fn(c > (d, e), a < b);
	fn(a <= b, c > (d, e));
	fn(a + b, c > (d, e));
	fn(g(a < b), c > (d, e));
	const b1 = { p: a < b, q: c > (d, e) };
	// A generic call written as one stays one
	fn(a<b, c>(d, e));
</script>

{fn((a < b), c > (d, e))}
<div data-attr={fn((a < b), c > (d, e))}></div>
{#if fn((a < b), c > (d, e))}<p>text</p>{/if}
