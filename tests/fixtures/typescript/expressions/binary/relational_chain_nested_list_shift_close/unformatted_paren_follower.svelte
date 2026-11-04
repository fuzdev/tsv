<script lang="ts">
	// A `<` behind a name whose would-be list closes INTO a shift token. The comparison
	// chain is valid there, so the list is read only where its body is a type: tsc and
	// acorn-typescript read each line below as comparisons and a shift
	const a1 = f < A < g(C) >> `t`;
	const a2 = f < A < -C >> `t`;
	const a3 = f < A < (C * D) >> `t`;
	const a4 = f < A < g(C) >> (t, u);
	const a5 = f < A < B < g(C) >>> `t`;
	const a6 = f < A < c.d() >> (t);

	// Controls: a single nest's `>>>` closes no list, and a type body keeps the list,
	// closed by the same shift
	const b1 = f < A < g(C) >>> `t`;
	const b2 = f<A<C>>`t`;
	const b3 = f<A<C | D>>(x);
	const b4 = new Map<string, Set<number>>();
</script>

<p>{f < A < g(C) >> `t`}</p>
