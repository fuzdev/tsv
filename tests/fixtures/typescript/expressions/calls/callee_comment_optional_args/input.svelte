<script lang="ts">
	// a block comment before `?.` stays on the callee side
	fn /* c1 */?.(a);

	// any argument count
	fn /* c2 */?.(a, b);

	// explicit type arguments — `?.` precedes them
	fn /* c3 */?.<A>(a);

	// authored after `?.`, the comment leads the first argument instead
	fn?.(/* c4 */ a);

	// authored after `?.` with type arguments, it leads the `<`
	fn?./* c5 */ <A>(a);

	// two comments on the callee side stay together
	fn /* c6 */ /* c7 */?.(a);

	// a non-optional call has no `?.` to split at
	fn(/* c8 */ a);

	// a call callee — the chain stays flat in both
	fn() /* c9 */?.(a);
</script>
