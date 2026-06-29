<script lang="ts">
	// own-line block comment between members (isolated from both) — expands one per line
	type A = X &
		/* a */
		Y;

	// own-line line comment between members — same expansion
	type B = X &
		// b
		Y;

	// own-line comment before the first member only — members stay inline
	type C =
		/* c */
		X & Y;

	// block comment inline-adjacent to the previous member — stays inline
	type D = X /* d */ & Y;

	// own-line comment on a middle member of a 3-member intersection — expands
	type E = X &
		/* e */
		Y &
		Z;

	// block comment inline-adjacent to the following member — stays inline
	type F = X & /* f */ Y;

	// blank line after an own-line comment is preserved
	type G = X &
		/* g */

		Y;
</script>
