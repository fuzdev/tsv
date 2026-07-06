<script lang="ts">
	// Computed class members separated by ASI (no semicolon between them)
	class C {
		[e]: number
		[e2]: number
	}

	// Computed field with initializer, then a computed member
	class D {
		[e] = id++
		[e2]: number
	}
</script>
