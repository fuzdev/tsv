<script lang="ts">
	// Line comment between two types
	type T = A &
		// comment before B
		B;

	// Line comment between multiple types
	type U = A &
		// comment before B
		B &
		// comment before C
		C;

	// Multiple consecutive line comments
	type V = A &
		// comment line 1
		// comment line 2
		B;
</script>
