<script lang="ts">
	// A pair around the comparison an element ends on
	xs.filter((x) => (x < max), (y) => y > (lo + hi) / 2);
	xs.filter(x => (x < max), y => y > (lo + hi) / 2);
	assert(ready && (lo < hi), max > (hi - lo) * step);
	fn(a ? b : (c < d), (e) => e > (f, g));
	fn(a || (c < d), (e) => e > (f, g));
	fn(a ?? (c < d), (e) => e > (f, g));
	fn(a == (c < d), (e) => e > (f, g));
	fn(x = (c < d), (e) => e > (f, g));
	fn((x) => (y) => (y < d), (e) => e > (f, g));
	function* gen() {
		fn(yield (c < d), (e) => e > (f, g));
	}
	// A pair around a comparison anywhere else in the element
	check(ok = (1 << bits) < limit, value >> (bits - 8));
	fn((c < d) ? e : f, (e) => e > (f, g));
	fn((c < d) || e, (e) => e > (f, g));
	fn(a ? (c < d) : e, (e) => e > (f, g));
	// Every pair on the way down to the comparison stays
	fn((x) => (a && (x < max)), (e) => e > (f, g));
	// Any element ahead of the closer, in any carrier
	fn(a, (x) => (x < max), b, (e) => e > (f, g));
	new Cls((x) => (x < max), (e) => e > (f, g));
	const a1 = [(x) => (x < max), (e) => e > (f, g)];
	// No pair is added where the author wrote none
	fn(c < d ? e : f, (e) => e > (f, g));
	fn(c < d || e, (e) => e > (f, g));
	fn(a ? c < d : e, (e) => e > (f, g));
	// A comparison inside a bracket of its own opens no region there, so the pair strips
	fn(g(h, (c < d)), (e) => e > (f, g));
	fn([(c < d)], (e) => e > (f, g));
	// With no closer later in the list the pair strips
	xs.filter((x) => (x < max), (y) => y);
</script>
