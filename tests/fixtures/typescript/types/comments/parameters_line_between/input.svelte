<script lang="ts">
	// Line comment between two type parameters
	type T<
		A,
		// comment before B
		B,
	> = A | B;

	// Line comment between multiple type parameters
	type U<
		A,
		// comment before B
		B,
		// comment before C
		C,
	> = A | B | C;

	// Multiple consecutive line comments
	type V<
		A,
		// comment line 1
		// comment line 2
		B,
	> = A | B;
</script>
