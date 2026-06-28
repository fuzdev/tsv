<script lang="ts">
	// Multi-line leading block comment before a union member: member breaks
	// onto its own line (source has a newline after the comment's */).
	type A =
		| /*
		   * doc1
		   */
		  a
		| b;

	// Same, with a parenthesized member.
	type B =
		| /*
		   * doc2
		   */
		  (c | d)
		| e;

	// Member glued on the */ line (no source newline): stays glued.
	type C =
		| /*
		   * doc3
		   */ f
		| g;

	// Single-line leading block comment: stays inline.
	type H = /* doc4 */ i | j;
</script>
