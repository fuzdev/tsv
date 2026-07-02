<script lang="ts">
	// After-comma block hugging the next item leads it (stays after the comma)
	interface A extends I,/* c */ J {}

	// Before-comma and after-comma blocks in one gap each keep their side
	interface B extends I /* c1 */,/* c2 */ J {}
</script>
