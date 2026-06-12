<script lang="ts">
	// Block comment after member (multiline)
	type A = {
		a: number;/* trailing */
	};

	// Line comment after member (multiline)
	type B = {
		b: string;// trailing
	};

	// Multiple members with trailing comments (multiline)
	type C = {
		a: number;/* first */
		b: string;// second
	};

	// Single member with trailing comment (single-line)
	type D = {a: number/* trailing */};

	// Multiple members with trailing comments (single-line)
	type E = {a: number;/* first */b: string/* second */};
</script>
