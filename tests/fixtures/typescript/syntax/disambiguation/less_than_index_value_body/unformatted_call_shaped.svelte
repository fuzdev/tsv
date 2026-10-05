<script lang="ts">
	// An index whose body is an EXPRESSION is a member access, so the `<` is the comparison
	// operator whatever follows the would-be closing `>` — a call, a tagged template, a
	// line break. Each line is the comparison the call-shaped authoring in the
	// `unformatted_call_shaped` variant reads as.
	const a1 = f<A[b + 1]>(x);
	const a2 = f<A[b()]>(x);
	const a3 = f<A[() => b + 1]>(x);
	const a4 = f<A[b ? c : d]>`t`;
	const a5 = f<A[-b]>(x);

	// The same body in a later member, and behind one.
	const b1 = a < b[c | d()] | e > (f);
	const b2 = f<A[b.c()] | B>(x);
	const b3 = f<B | A[b.c()]>(x);
	const b4 = f<A['a' + b] | B>(x);
</script>
