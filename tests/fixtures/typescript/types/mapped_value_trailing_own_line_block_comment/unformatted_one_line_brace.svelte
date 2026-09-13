<script lang="ts">
	// a block the author gave its own line after the member `;` keeps that line, as
	// a type literal's own-line comment after its last member does
	type A = {[K in T]: V
		/* c1 */};

	// a multi-line block answers the same way
	type B = {[K in T]: V
		/* c2
c3 */};

	// a run of own-line blocks stays one per line
	type C = {[K in T]: V
		/* c4 */
		/* c5 */};

	// null control: a block on the member's line trails the value inline, before the `;`
	type D = {
		[K in T]: V /* c6 */;
	};

	// null control: a multi-line block on the member's line does too
	type E = {
		[K in T]: V /* c7
c8 */;
	};

	// a block glued behind an own-line block rides the deferred run with it, the pair
	// keeping its line
	type F = {[K in T]: V
		/* c9 */ /* c10 */};

	// a line comment answers the same way, and a one-line brace shell that holds any
	// deferred trailing run breaks so the run lands inside the braces
	type G = {[K in T]: V // c11
	};
</script>
