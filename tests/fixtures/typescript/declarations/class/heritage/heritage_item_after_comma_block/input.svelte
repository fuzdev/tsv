<script lang="ts">
	// After-comma block hugging the next item leads it (stays after the comma)
	class A implements I, /* c */ J {}

	// Before-comma and after-comma blocks in one gap each keep their side
	class B implements I /* c1 */, /* c2 */ J {}
</script>
