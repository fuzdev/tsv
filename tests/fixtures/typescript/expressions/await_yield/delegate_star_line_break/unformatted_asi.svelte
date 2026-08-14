<script>
	function* fn() {
		// `yield [no LineTerminator here] *` — the delegate `*` binds to `yield` only on
		// the same line, and a bare `*` cannot start a statement, so a break has no valid
		// reading at all. The valid baseline keeps the `*` glued to its keyword.
		yield* a;
		// The non-delegate half has a valid reading and demotes instead: ASI ends the
		// `yield` at the newline and the operand becomes its own statement.
		yield
		b;
	}
</script>
