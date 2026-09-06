<script lang="ts">
	// An own-line block run whose LAST comment the author glued to the value: the run keeps
	// its own lines, and the glued comment stays on the value's line (a block comment followed
	// by a newline ends its line; one followed by the value does not).
	type A =
		/* c1 */
		/* c2 */ Ref;

	// a glued pair at the end of the run stays a pair
	type B =
		/* c1 */
		/* c2 */ /* c3 */ Ref;

	// a line comment above the glued block
	type C =
		// c1
		/* c2 */ Ref;

	// a multiline block glued to the value
	type D =
		/* c1 */
		/* c2
		   c3 */ Ref;

	// the value kind doesn't matter: a union, a conditional
	type E =
		/* c1 */
		| /* c2 */ Ref1
		| Ref2;
	type F =
		/* c1 */
		/* c2 */ Ref extends Check ? True : False;

	// contrast: the last comment followed by a newline keeps its own line
	type G =
		/* c1 */
		/* c2 */
		Ref;
</script>
