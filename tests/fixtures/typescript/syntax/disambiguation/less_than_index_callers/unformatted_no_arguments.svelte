<script lang="ts">
	// A type-argument list whose first argument is an indexed access continued by `|` or
	// `&` is the same list after any callee, before an argument list or a template, and in
	// any expression position.
	fn<A[K] | B>(x);
	fn<A[K] | B, C>(x);
	fn<A[K] | B>(x).y;
	fn<A[K] | B>(x)(y);
	fn<A[K] | B>`t`;
	fn<A[K] | B>`a${x}b`;
	obj.fn<A[K] | B>(x);
	obj?.fn<A[K] | B>(x);
	obj[0]<A[K] | B>(x);
	fn()<A[K] | B>(x);
	new fn<A[K] | B>(x);
	new obj.fn<A[K] | B>(x);
	new fn<A[K] | B>`t`;

	// Expression positions.
	g(fn<A[K] | B>(x), y);
	const a1 = () => fn<A[K] | B>(x);
	const a2 = cond ? fn<A[K] | B>(x) : fn<A[K] & B>(y);
	const a3 = [fn<A[K] | B>(x)];
	const a4 = !fn<A[K] | B>(x);
	const a5 = typeof fn<A[K] | B>(x);
	const a6 = a ?? fn<A[K] | B>(x);
	async function f1() {
		return await fn<A[K] | B>(x);
	}
	if (fn<A[K] | B>(x)) {
	}
</script>

{fn<A[K] | B>(x)}
{fn<A[0] & B>(x)}
{obj.fn<this[K] | B>`t`}
<div data-attr={fn<A[K] | B>(x)}></div>
{#if fn<A[K] | B>(x)}{/if}
