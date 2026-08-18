<script lang="ts">
	// A `//` glued to the required head pair's `(` keeps that line; the pair expands.
	const i = // c1
	(<T extends string>(p: T) => p)<string>;

	// An own-line comment keeps its own line inside the pair.
	// c2
	(<T extends string>(p: T) => p)<string>;

	// A glued block leads the head inline; the pair stays flat.
	/* c3 */ (<T extends string>(p: T) => p)<string>;
</script>
