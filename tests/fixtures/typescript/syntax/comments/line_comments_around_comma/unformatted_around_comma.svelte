<script lang="ts">
	// Only one line comment can trail the comma - the second leads the next item
	type T = [
		A // c1
		, // c2
		B
	];

	// Function parameters
	function fn(
		a: A // c1
		, // c2
		b: B
	) {}

	// Type parameters
	function fn2<
		T // c1
		, // c2
		U
	>() {}

	// Function type parameters
	type F = (
		a: A // c1
		, // c2
		b: B
	) => void;
</script>
