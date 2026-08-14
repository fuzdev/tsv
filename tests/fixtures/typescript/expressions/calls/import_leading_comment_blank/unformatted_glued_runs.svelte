<script lang="ts">
	// A blank the author left after a leading comment is theirs; the comment still
	// leads the source argument.
	const a = import(
		/* c1 */

		'./a'
	);

	// The same with a line comment.
	const b = import(
		// c2

		'./a'
	);

	// A blank between two leading comments.
	const c = import(
		// c3

		// c4
		'./a'
	);

	// An options argument does not change the leading gap's rule.
	const d = import(
		/* c5 */

		'./a',
		{ with: { type: 'json' } }
	);

	// A run the author glued stays glued, and glue alone opens no parens.
	const e = import(
		/* c6 */ /* c7 */
		'./a'
	);

	// A glued run whose line comment ends the line keeps the pair on one line.
	const f = import(
		/* c8 */    // c9
		'./a'
	);

	// A blank after a glued run is the author's too.
	const g = import(
		/* c10 */ /* c11 */

		'./a'
	);

	// A TS import type shares the leading-gap emitter, so it takes the same rule.
	type A = import(
		/* c12 */

		'./a'
	).Foo;
</script>
