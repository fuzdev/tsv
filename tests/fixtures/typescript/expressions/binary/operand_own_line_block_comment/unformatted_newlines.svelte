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

	// same-line block comment stays inline (no break)
	const d = x + /* d */ y;
</script>
