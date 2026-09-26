<script lang="ts">
	// The list followed by no argument expression at all — an empty argument list, an
	// optional call, an operator, a member, or nothing (an instantiation expression) — is
	// the same type-argument list, and the only reading there is.
	fn<A[K] | B>();
	fn<A[K] & B>?.(x);
	new fn<A[K] | B>;
	fn<A[K] | B>;
	fn<A[K] | B> == c;
	fn<A[K] | B> ?? c;
	(fn<A[K] | B>)[x];
	const a1 = fn<A[K] | B>;
	const a2 = fn<A[0] & B>;
	g(fn<A[K] | B>, y);
	const a3 = [fn<A[K] | B>];
	const a4 = { a: fn<A[K] | B> };
	const a5 = cond ? fn<A[K] | B> : fn<A[K] & B>;
</script>

{fn<A[K] | B>}
{fn<A[K] | B>()}
