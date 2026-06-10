<script lang="ts">
	// line comment trailing type-argument `<` - two arguments
	type A = Map< // c1
		string,
		number
	>;

	// line comment trailing type-argument `<` - three arguments
	type B = Triple< // c2
		string,
		number,
		boolean
	>;

	// block comment trailing `<`, then own-line leading comment
	type C = Map< /* angle */
		// lead
		string,
		number
	>;
</script>
