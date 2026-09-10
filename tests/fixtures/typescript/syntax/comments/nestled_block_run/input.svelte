<script lang="ts">
	// Two block comments the author left BYTE-ADJACENT, both indentable (every line of
	// the pair starts with `*`), are ONE comment: no separator comes between them at any
	// position. One byte of gap, or either comment not indentable, and they are two
	// (separator_glued_run, body_dangling_run).

	// Leading a statement.
	/** a
	 *//** b
	 */
	const a = 1;

	// Leading an object member.
	const b = {
		/** a
		 *//** b
		 */
		c: 1
	};

	// Trailing an object member.
	const d = {
		e: 1
		/** a
		 *//** b
		 */
	};

	// Dangling - the only content of an object literal.
	const f = {
		/** a
		 *//** b
		 */
	};

	// Dangling - a class body.
	class A {
		/** a
		 *//** b
		 */
	}

	// Dangling - an interface body.
	interface B {
		/** a
		 *//** b
		 */
	}

	// Dangling - a parameter list.
	function fn(
		/** a
		 *//** b
		 */
	) {}

	// Leading an argument.
	fn(
		/** a
		 *//** b
		 */ g
	);

	// Three in a row.
	/** a
	 *//** b
	 *//** c
	 */
	const h = 1;

	// A gap after the pair leaves the third its own comment.
	/** a
	 *//** b
	 */ /** c
	 */
	const i = 1;

	// The `/**` spelling is not the rule - a `*`-aligned `/*` nestles too.
	/*
	 * a
	 *//*
	 * b
	 */
	const j = 1;

	// Control - one byte of gap and the pair is two comments.
	/** a
	 */ /** b
	 */
	const k = 1;

	// Control - a newline between them keeps two lines.
	/** a
	 */
	/** b
	 */
	const l = 1;

	// Control - single-line blocks are not indentable, so they stay two.
	/* a */ /* b */
	const m = 1;

	// Control - a multi-line block whose lines lack `*` is not indentable.
	/* a
	z */ /* b
	z */
	const n = 1;

	// Control - the pair must BOTH be indentable.
	/** a
	 */ /* b */
	const o = 1;

	// Trailing at the end of the body.
	const p = 1;
	/** a
	 *//** b
	 */
</script>
