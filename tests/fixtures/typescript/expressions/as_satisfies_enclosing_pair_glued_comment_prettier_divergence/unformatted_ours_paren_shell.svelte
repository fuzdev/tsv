<script lang="ts">
	// A block comment glued to a cast's operand stays glued to it when the cast takes a
	// pair of its own: the pair goes outside the comment.

	// member base
	const a = ((/* c1 */ (x)) as A).b;

	// callee
	const b = ((/* c2 */ (x)) as A)();

	// logical operand
	const c = y && ((/* c3 */ (x)) as A);

	// ternary test
	const d = ((/* c4 */ (x)) satisfies A) ? 1 : 2;

	// non-null operand
	const e = ((/* c5 */ (x)) as A)!;

	// null control: no pair to add, both formatters agree
	const f = (/* c6 */ (x)) as A;

	// a second member base, which the shell variant leaves bare
	const g = (/* c7 */ x as A).c;
</script>
