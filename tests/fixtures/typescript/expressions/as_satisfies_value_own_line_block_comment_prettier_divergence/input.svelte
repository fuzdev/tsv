<script lang="ts">
	// Own-line block comment after `as`, before the type: kept on its own line,
	// type on the next line (tsv preserves the author's placement).
	const a = x as
		/* a */
		A;

	// `satisfies` behaves the same.
	const b = x satisfies
		/* b */
		B;

	// A blank line after the comment is preserved.
	const c = x as
		/* c */

		C;

	// A same-line block comment with the type authored on the next line keeps the
	// type on that line (comment trailing the keyword); preserving the author's break.
	const d = x as /* d */
		D;

	// A same-line block comment with the type glued to it stays fully inline (not a
	// divergence — both formatters keep it).
	const e = x as /* e */ E;
</script>
