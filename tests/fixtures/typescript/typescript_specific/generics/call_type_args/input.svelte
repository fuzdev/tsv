<script lang="ts">
	// Simple type arg
	const a = fn<string>();

	// Union type arg
	const b = fn<T | null>();
	const c = fn<A | B | C>();

	// Intersection type arg
	const d = fn<A & B>();

	// Multiple type args
	const e = fn<T, U>();
	const f = fn<T | null, U & V>();

	// Nested generics
	const g = fn<Array<T>>();
	const h = fn<Map<K, V | null>>();

	// With arguments
	const i = fn<T>(a);
	const j = fn<T | null>(null);
</script>
