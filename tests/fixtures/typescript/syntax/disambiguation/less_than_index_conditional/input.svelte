<script lang="ts">
	// An indexed-access type argument can be the check type of a conditional: `extends`
	// after the index continues the type, whatever the index spells.
	const a1 = fn<A[K] extends B ? C : D>(x);
	const a2 = fn<A[0] extends B ? C : D>(x);
	const a3 = fn<A[number] extends B ? C : D>(x);
	const a4 = fn<this[K] extends B ? C : D>(x);
	const a5 = fn<A[K] extends infer U ? U : D>(x);
	const a6 = fn<A[K] extends B ? C : D, E>(x);

	// A conditional type indexed, then continued.
	const b1 = fn<(A extends B ? C : D)[K] | E>(x);
	const b2 = fn<(A extends B ? C : D)[K] extends E ? F : G>(x);

	// The check type committing on its own: a string key, a chained index, a plain name.
	const c1 = fn<A['k'] extends B ? C : D>(x);
	const c2 = fn<A[K][J] extends B ? C : D>(x);
	const c3 = fn<B extends A[K] ? C : D>(x);
	const c4 = fn<(A extends B ? C : D)[K]>(x);
</script>
