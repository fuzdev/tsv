<script lang="ts">
	// A comment the author GLUED to the previous trailing comment's `*/` keeps that line,
	// whatever put that line out of the argument's reach (here a multiline block). The same
	// rule a trailing gap reads (syntax/comments/trailing_gap_glued_run), at the argument
	// lists.

	// Call argument.
	fn(
		a /* c1
c2 */ // c3
	);

	// `new` argument.
	new Fn(
		a /* c1
c2 */ // c3
	);

	// Dynamic import options.
	import(
		'./a',
		a /* c1
c2 */ // c3
	);

	// Member-chain call.
	obj.fn().fn(
		a /* c1
c2 */ // c3
	);

	// A comment glued to a block the printer itself pulls onto the argument's line comes
	// along - the deleted-comma sanction stops at that block
	// (syntax/comments/last_item_line_after_comma_prettier_divergence).
	fn4(
		a /* b */ // c
	);

	// Dynamic import with no options - the sole argument's gap holds no comma, and the
	// claim is the same one.
	import(
		'./b' /* c1
c2 */ // c3
	);

	// Import-equals `require` reference.
	import a2 = require(
		'./c' /* c1
c2 */ // c3
	);

	// Control - a comment the author gave a line of its own keeps it.
	fn2(
		a /* c1
c2 */
		// c3
	);

	// Control - a glued block pair, claimed by the source reading either way.
	fn3(
		a /* c1
c2 */ /* c3 */
		// c4
	);
</script>
