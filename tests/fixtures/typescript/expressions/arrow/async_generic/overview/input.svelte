<script lang="ts">
	// Async generic arrow functions
	const fetchItem = async <T,>(id: string): Promise<T> => ({}) as T;
	const transform = async <T, U>(x: T, fn: (t: T) => U): Promise<U> => fn(x);
	const constrained = async <T extends object>(x: T) => x;
	const withDefault = async <T = string,>(x: T) => x;
</script>
