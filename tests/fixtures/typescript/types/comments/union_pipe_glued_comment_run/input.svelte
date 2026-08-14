<script lang="ts">
	// a glued run after the `|` whose last comment breaks to the member goes own-line
	// above the `|`, in source order and still glued on one line
	type A =
		| a // t
		/* c1 */ /* c2 */
		| b;

	// the whole run moves, not just its last comment
	type B =
		| a // t
		/* c1 */ /* c2 */ /* c3 */
		| b;

	// an own-line comment ahead of a glued pair keeps its own line, and the pair stays
	// together rather than splitting across the `|`
	type C =
		| a // t
		/* c1 */
		/* c2 */ /* c3 */
		| b;

	// control: a run glued all the way to the member stays after the `| `
	type D =
		| a // t
		| /* c1 */ b;

	type E =
		| a // t
		| /* c1 */ /* c2 */ b;
</script>
