<script lang="ts">
	// line comment trailing a class body `{`
	class C { // c1
		a = 1;
	}

	// block comment trailing the body `{`, then own-line leading comment
	class D { /* brace */
		// lead
		b = 2;
	}

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap
	class E { // c2

		c = 3;
	}

	// the same, with the block spelling (an own-line comment is what pulls a block onto
	// the `{` line)
	class F { /* brace2 */

		// lead
		d = 4;
	}

	// the blank ABOVE the comment is a different question and stays erased — it sits
	// against the delimiter, where both formatters drop it at every bracket
	class G {
		// c3
		e = 5;
	}
</script>
