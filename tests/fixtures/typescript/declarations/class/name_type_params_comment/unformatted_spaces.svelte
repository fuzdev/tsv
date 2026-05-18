<script lang="ts">
	// Block comment between class name and type params
	class A /* c1 */ <T> {}

	// Class expression - block comment between name and type params
	const a = class B /* c2 */ <T> {};
</script>
