<script lang="ts">
	// Function type as return type - requires parentheses for disambiguation
	const f =
		(x: T): ((y: T) => T) =>
		(y) =>
			y;

	// Empty params function type
	const g =
		(x: T): (() => T) =>
		() =>
			x;

	// Complex function type with multiple params
	const h =
		(x: T, y: U): ((a: T, b: U) => [T, U]) =>
		(a, b) => [a, b];
</script>
