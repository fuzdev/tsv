<script lang="ts">
	// A member's `;` deferred a line comment past itself, so a following comment on the
	// `;`'s line takes its own line below rather than joining the deferred one.
	class C1 {
		a = 1 // c1
		; // c2
		b = 2;
	}

	// A block there keeps its authored position after the deferred line comment.
	class C2 {
		a = 1 // c3
		; /* c4 */
		b = 2;
	}

	// The same rule in an interface body.
	interface I1 {
		a: A // c5
		; // c6
		b: B;
	}

	interface I2 {
		a: A // c7
		; /* c8 */
		b: B;
	}

	// And in a type literal.
	type T1 = {
		a: A // c9
		; // c10
		b: B;
	};

	type T2 = {
		a: A // c11
		; /* c12 */
		b: B;
	};

	// The deferred run may be an own-line BLOCK rather than the trailing line comment;
	// what follows the `;` still takes the line below it, after that block.
	type T3 = {
		a: A // c13
		/* c14 */; /* c15 */
		b: B;
	};

	// On the LAST member of a body there is no following member to lead the comment, so
	// the body's own end-of-run emitter is the one that claims it.
	class C3 {
		a = 1 // c16
		; // c17
	}

	interface I3 {
		a: A // c18
		; // c19
	}

	type T4 = {
		a: A // c20
		; // c21
	};
</script>
