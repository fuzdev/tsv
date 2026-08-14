<script lang="ts">
	// own-line block comment before a binary operand keeps the operand on its own line
	const a =
		x +
		/* a */
		y;

	// logical operand: same rule
	const b =
		x &&
		/* b */
		y;

	// blank line after the comment is preserved
	const c =
		x ||
		/* c */

		y;

	// blank line before the comment is dropped - it sits above a leading run, where
	// prettier has no emitter for it
	const e =
		x ||
		// e
		y;

	// same-line block comment stays inline (no break)
	const d = x + /* d */ y;
</script>
