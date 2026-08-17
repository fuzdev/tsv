<script lang="ts">
	// line comment trailing an interface body `{`
	interface I { // c1
		a: number;
	}

	// block comment trailing the body `{`, then own-line leading comment
	interface J { /* brace */
		// lead
		b: string;
	}

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap
	interface K { // c2

		c: number;
	}

	// the same, with the block spelling (an own-line comment is what pulls a block onto
	// the `{` line)
	interface L { /* brace2 */

		// lead
		d: string;
	}

	// the blank ABOVE the comment is a different question and stays erased — it sits
	// against the delimiter, where both formatters drop it at every bracket
	interface M {
		// c3
		e: boolean;
	}
</script>
