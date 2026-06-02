<script lang="ts">
	// line comment trailing `<` - function declaration
	function fn<
		// c
		T,
	>(p: T) {}

	// line comment trailing `<` - class
	class C<
		// c
		T,
	> {}

	// line comment trailing `<` - interface
	interface I<
		// c
		T,
	> {}

	// line comment trailing `<` - type alias
	type A<
		// c
		T,
	> = T;

	// line comment trailing `<` - arrow function
	const g = <
		// c
		T,
	>(
		p: T,
	): T => p;
</script>
