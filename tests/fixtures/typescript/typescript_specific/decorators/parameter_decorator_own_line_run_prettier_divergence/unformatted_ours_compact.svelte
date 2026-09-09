<script lang="ts">
	// An own-line comment run in a decorator gap keeps its own line, whether or not a
	// newline directly follows a decorator.

	class Boundary {
		// One comment: prettier keeps its line here too
		single(@fn1 /* t */
		/* c */
		@fn2 p) {}

		// Two comments the author glued onto one line: prettier collapses the run
		pair(@fn1 /* t */
		/* c1 */ /* c2 */
		@fn2 p) {}

		// The class MEMBER twin of the same authoring, where prettier keeps the run
		@fn1 /* t */
		/* c1 */ /* c2 */
		@fn2 a = 1;
	}

	// The class-LEVEL twin, where prettier keeps it too
	@fn1 /* t */
	/* c1 */ /* c2 */
	@fn2
	class Level {}
</script>
