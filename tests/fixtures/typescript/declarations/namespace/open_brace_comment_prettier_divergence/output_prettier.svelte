<script lang="ts">
	// line comment trailing a namespace body `{` - single statement
	namespace N1 {
		// c1
		const a = 1;
	}

	// line comment trailing a namespace body `{` - multiple statements
	namespace N2 {
		// c2
		const b = 2;
		const c = 3;
	}

	// block comment trailing the body `{`, then own-line leading comment
	namespace N3 {
		/* brace */
		// lead
		const d = 4;
	}

	// line comment trailing a `module` body `{`
	module M1 {
		// c3
		const e = 5;
	}

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap
	namespace N4 {
		// c4

		const f = 6;
	}

	// the same, with the block spelling (an own-line comment is what pulls a block onto
	// the `{` line)
	namespace N5 {
		/* brace2 */

		// lead
		const g = 7;
	}

	// the blank ABOVE the comment is a different question and stays erased — it sits
	// against the delimiter, where both formatters drop it at every bracket
	namespace N6 {
		// c5
		const h = 8;
	}
</script>
