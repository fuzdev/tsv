<script lang="ts">
	// line comment trailing a type literal `{` - single member
	type A = {
		// c1
		a: number;
	};

	// line comment trailing a type literal `{` - multiple members
	type B = {
		// c2
		b: number;
		c: string;
	};

	// block comment trailing the `{`, then own-line leading comment
	type C = {
		/* brace */
		// lead
		d: boolean;
	};

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap
	type D = {
		// c3

		e: number;
	};

	// the same, with the block spelling (an own-line comment is what pulls a block onto
	// the `{` line)
	type E = {
		/* brace2 */

		// lead
		f: string;
	};

	// the blank ABOVE the comment is a different question and stays erased — it sits
	// against the delimiter, where both formatters drop it at every bracket
	type F = {
		// c4
		g: boolean;
	};
</script>
