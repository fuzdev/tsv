<script lang="ts">
	interface Store<T> {
		readonly subscribe(run: (value: T) => void): () => void;
		readonly reset?(): void;
		readonly map<U>(fn: (value: T) => U): Store<U>;
		readonly [Symbol.iterator](): Iterator<T>;
		readonly 'on-change'(): void;
	}

	type Handle = { readonly close(): void };
</script>
