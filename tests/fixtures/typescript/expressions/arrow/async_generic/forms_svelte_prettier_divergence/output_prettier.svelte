<script lang="ts">
	// Async generic arrow forms that each exercise a path beyond the canonical
	// stacked case (see stacked_svelte_prettier_divergence). Every single
	// unconstrained `<T>` stays bare under tsv; prettier forces `<T,>` (output_prettier).

	// Optional param — the acorn-typescript param-drop hits `x?` too, a distinct
	// param node from the plain/rest forms. tsv keeps it (expected_ours); Svelte
	// drops it (expected_svelte). Single `<T>` also takes prettier's `<T,>`.
	const withOptional = async <T,>(x?: T): Promise<T | undefined> => x;

	// Object-literal body with `as` assertion — no params, so nothing to drop;
	// only the `<T,>` formatter divergence applies.
	const objectBody = async <T,>(): Promise<T> => ({}) as T;

	// Typed binding — the annotation `<T>` is a type position and stays bare in
	// BOTH tools; only the initializer `<T>` (value position) takes `<T,>`.
	const typed: <T>() => Promise<T> = async <T,>() => ({}) as T;
</script>
