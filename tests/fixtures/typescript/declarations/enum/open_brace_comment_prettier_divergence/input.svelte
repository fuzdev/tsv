<script lang="ts">
	// line comment trailing an enum body `{`
	enum E { // c1
		A
	}

	// block comment trailing the body `{`, then own-line leading comment
	enum F { /* brace */
		// lead
		B
	}

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap
	enum G { // c2

		C
	}

	// the same, with the block spelling (an own-line comment is what pulls a block onto
	// the `{` line)
	enum H { /* brace2 */

		// lead
		D
	}

	// the blank ABOVE the comment is a different question and stays erased — it sits
	// against the delimiter, where both formatters drop it at every bracket
	enum I {
		// c3
		E
	}
</script>
