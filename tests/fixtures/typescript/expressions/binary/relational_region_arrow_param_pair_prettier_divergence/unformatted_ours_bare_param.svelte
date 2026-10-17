<script lang="ts">
	// Between a sibling holding a `<` and a later sibling's `>` that may close the region, an arrow
	// function's lone parameter written without a pair prints without one
	check(lo < hi, [x => x > (lo + hi) / 2]);
	fn(a < b, { q: c => c > (d, e) });
	// A list element of its own, between the two comparisons or holding the closer
	xs.filter(
		x => x < max,
		y => y > (lo + hi) / 2
	);
	fn(a < b, c => c > (d - e) * 2);
	fn(a < b, c => d, e > (f, g));
	fn(a < b, [c => d], e > (f, g));
	// The carriers
	new Cls(a < b, [c => c > (d, e)]);
	fn?.(a < b, [c => c > (d, e)]);
	const a1 = [a < b, { q: c => c > (d, e) }];
	(a < b, [c => c > (d, e)]);
	const a2 = `${(a < b, [c => c > (d, e)])}`;
	fn(a < b, `${c => c > (d, e)}`);
	// The arrow ahead of the closer, after another element, curried, behind `async`
	fn(a < b, [c => c, d > (e, f)]);
	fn(a < b, { q: c => c, r: d > (e, f) });
	fn(a < b, [y, c => c > (d, e)]);
	fn(a < b, [c => (d) => d > (e, f)]);
	fn(a < b, [async c => c > (d, e)]);
	// The follower is not read
	fn(a < b, [c => c > (d - e) * 2]);
	fn(a < b, [c => c > `t`]);
	fn(a < b, [c => c > d]);
	// The region is the parser's flag, so a `<` side no list can be read from keeps it bare too
	fn(a < b + 1, [c => c > (d, e)]);
	// The pair stays where no later sibling holds a closer or no sibling opens a region
	fn(a < b, [c => c]);
	fn(a < b, [c => c >= d]);
	fn(a <= b, [c => c > (d, e)]);
	fn(a + b, [c => c > (d, e)]);
	fn(g(a < b), [c => c > (d, e)]);
	fn([c => c > (d, e)], a < b);
	const b1 = { p: a < b, q: c => c > (d, e) };
</script>

{check(lo < hi, [x => x > (lo + hi) / 2])}
<div data-attr={fn(a < b, [c => c > (d, e)])}></div>
{#if fn(a < b, [c => c, d > (e, f)])}<p>text</p>{/if}
