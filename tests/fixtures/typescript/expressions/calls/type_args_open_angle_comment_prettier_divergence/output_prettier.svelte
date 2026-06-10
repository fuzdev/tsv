<script lang="ts">
	// line comment trailing type-argument `<` — two arguments
	foo<
		// c1
		A,
		B
	>(x);

	// line comment trailing type-argument `<` — three arguments
	bar<
		// c2
		A,
		B,
		C
	>(x);

	// block comment trailing `<`, then own-line leading comment
	baz<
		/* angle */
		// lead
		A,
		B
	>(x);
</script>
