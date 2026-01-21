<script lang="ts">
	// Line comment between two type arguments
	type T = Map<
		A,
		// comment before B
		B
	>;

	// Line comment between multiple type arguments
	type U = Generic<
		A,
		// comment before B
		B,
		// comment before C
		C
	>;

	// Multiple consecutive line comments
	type V = Map<
		A,
		// comment line 1
		// comment line 2
		B
	>;
</script>
