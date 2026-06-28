<script lang="ts">
	// own-line block comment before each member — union expands one per line
	type A =
		/* a */
		| 'x'
		/* b */
		| 'y';

	// own-line line comment before each member — same expansion
	type B =
		// c
		| 'x'
		// d
		| 'y';

	// own-line comment on the first member only — stays inline
	type C =
		/* e */
		'x' | 'y';

	// same-line block comment between members — stays inline (no own-line comment)
	type D = 'x' /* f */ | 'y';

	// own-line comment on a middle member of a 3-member union — expands
	type E =
		| 'x'
		/* g */
		| 'y'
		| 'z';
</script>
