<script lang="ts">
	// line comment trailing a type literal `{` - single member
	type A = { // c1
		a: number;
	};

	// line comment trailing a type literal `{` - multiple members
	type B = { // c2
		b: number;
		c: string;
	};

	// block comment trailing the `{`, then own-line leading comment
	type C = { /* brace */
		// lead
		d: boolean;
	};
</script>
