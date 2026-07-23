<script lang="ts">
	// A `//` comment authored on the `(` line of a yield argument stays trailing the
	// `(`; prettier relocates it to trail the `yield` keyword with the argument below.
	// The comment forces the parens to break either way — the only question is whether
	// it leads the argument line or trails the `(`. The yield analog of the return/throw
	// keyword_open_paren_line_comment fixture (the third restricted production).
	function* assign() {
		yield // c
		(a = b);
	}
	function* seq() {
		yield // c
		(a, b);
	}
	function* ident() {
		yield // c
		a;
	}
	function* delegate() {
		yield* // c
		a;
	}
</script>
