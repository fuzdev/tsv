<script lang="ts">
	// an own-line BLOCK inside a union member's stripped paren shell keeps its own line,
	// exactly as the `//` spelling and the shell-free authoring do: above the `| ` at a
	// later member, at the member offset at the first

	// a later member
	type A =
		| B
		/* c1 */
		| C;

	// the member is an object type
	type D =
		| E
		/* c2 */
		| { a: 1 };

	// two own-line blocks, each keeping its own line
	type F =
		| G
		/* c3 */
		/* c4 */
		| H;

	// the second block glued to the member: one isolated comment takes the whole run out
	type I =
		| J
		/* c5 */
		/* c6 */
		| K;

	// the FIRST member: a glued block, then one the author gave its own line
	type L =
		| /* c7 */
		  /* c8 */
		  M
		| N;

	// control: the `//` spelling, which lands on the same shape
	type O =
		| P
		// c9
		| Q;

	// control: a block the author glued to the `(` stays inline
	type R = S | /* c10 */ T;
</script>
