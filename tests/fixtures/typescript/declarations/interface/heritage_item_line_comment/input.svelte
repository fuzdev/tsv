<script lang="ts">
	// Line comment after extends item in interface
	interface A
		extends
			I, // c
			J {}

	// Multiple extends items with line comment in middle
	interface B
		extends
			I, // c
			J,
			K {}
</script>
