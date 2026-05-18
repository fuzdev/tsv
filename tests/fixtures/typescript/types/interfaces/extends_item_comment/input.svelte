<script lang="ts">
	// Block comment after extends item
	interface A extends B /* c1 */, C {}

	// Multiple extends with block comments
	interface D extends E /* c2 */, F /* c3 */, G {}
</script>
