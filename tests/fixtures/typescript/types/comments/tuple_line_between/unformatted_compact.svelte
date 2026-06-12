<script lang="ts">
	// Line comment between two tuple elements
	type T = [A,
		// comment before B
		B];

	// Line comment between multiple tuple elements
	type U = [A,
		// comment before B
		B,
		// comment before C
		C];

	// Multiple consecutive line comments
	type V = [A,
		// comment line 1
		// comment line 2
		B];
</script>
