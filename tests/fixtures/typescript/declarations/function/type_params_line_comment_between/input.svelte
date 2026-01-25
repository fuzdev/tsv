<script lang="ts">
	// Line comment between two function type parameters
	type T = (
		a: A,
		// comment before b
		b: B,
	) => C;

	// Line comment between multiple function type parameters
	type U = (
		a: A,
		// comment before b
		b: B,
		// comment before c
		c: C,
	) => D;

	// Multiple consecutive line comments
	type V = (
		a: A,
		// comment line 1
		// comment line 2
		b: B,
	) => C;
</script>
