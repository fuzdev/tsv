<script lang="ts">
	// Multiple line comments between extends items
	interface A
		extends
			I, // c1
			// c2
			J {}

	// Three line comments between extends items
	interface B
		extends
			I, // c1
			// c2
			// c3
			J {}
</script>
