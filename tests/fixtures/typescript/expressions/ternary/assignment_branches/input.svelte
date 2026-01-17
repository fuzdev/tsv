<script lang="ts">
	// Both branches have assignments with calls
	const a = cond ? (b = c.map((x) => x.d)) : (e = f.filter((x) => x.g));

	// Nested ternaries with assignments
	const h = cond1
		? (i = j.map((x) => x))
		: cond2
			? (k = l.filter((x) => x))
			: (m = n.reduce((a, b) => a + b, 0));

	// Assignment in test expression too
	const o = (p = q.some((x) => x.r)) ? (s = t.map((x) => x.fn())) : (u = v.filter((x) => x.w));

	// Long assignments that break
	const veryLongVariableNameThatExceeds = someReallyLongConditionThatBreaks
		? (aa = bb.map((x) => x.fn()))
		: (cc = dd.filter((x) => x.ee));

	// Assignment in consequent only
	const ff = cond3 ? (gg = hh.map((x) => x.ii)) : jj;

	// Multiple levels of parenthesized assignments
	const kk =
		cond4 && cond5
			? (ll = mm.map((x) => (nn = x.filter((y) => y))))
			: (oo = pp.filter((x) => (qq = x.map((y) => y))));
</script>
