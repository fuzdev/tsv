<script lang="ts">
	// Type reference arguments: line comment trailing an argument
	type T = Foo<
		A // c
		, B
	>;

	// Call type arguments
	const x = fn<
		A // c
		, B
	>();
</script>
