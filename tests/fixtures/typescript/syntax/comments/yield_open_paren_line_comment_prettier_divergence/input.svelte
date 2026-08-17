<script lang="ts">
	// A `//` comment authored on the `(` line of a yield argument stays trailing the
	// `(`; prettier relocates it to trail the `yield` keyword with the argument below.
	// The comment forces the parens to break either way — the only question is whether
	// it leads the argument line or trails the `(`. The yield analog of the return/throw
	// keyword_open_paren_line_comment fixture (the third restricted production).
	function* assign() {
		yield ( // c
			a = b
		);
	}
	function* seq() {
		yield ( // c
			a, b
		);
	}
	function* ident() {
		yield ( // c
			a
		);
	}
	function* delegate() {
		yield* ( // c
			a
		);
	}

	// The OWN-LINE authoring diverges too — unlike the return/throw sibling, where it
	// matches. Prettier relocates the comment onto the keyword line and drops the parens
	// from either authoring, so yield has one prettier form and two tsv fixed points.
	function* assignOwnLine() {
		yield (
			// c
			a = b
		);
	}
	function* seqOwnLine() {
		yield (
			// c
			a, b
		);
	}
	function* identOwnLine() {
		yield (
			// c
			a
		);
	}
	function* delegateOwnLine() {
		yield* (
			// c
			a
		);
	}

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap (the blank ABOVE one stays erased, against the delimiter)
	function* blank() {
		yield ( // c

			a
		);
	}
</script>
