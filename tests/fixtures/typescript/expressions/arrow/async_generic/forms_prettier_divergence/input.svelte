<script lang="ts">
	// Async generic arrow forms that each exercise a path beyond the canonical
	// minimal case (see minimal_prettier_divergence). Every single
	// unconstrained `<T>` stays bare under tsv; prettier forces `<T,>` (output_prettier).

	// Optional param — a distinct param node from the plain form, on an arrow
	// whose return type is a union. Single `<T>` takes prettier's `<T,>`.
	const withOptional = async <T>(x?: T): Promise<T | undefined> => x;

	// Object-literal body with `as` assertion — a parenthesized object body,
	// where only the `<T,>` formatter divergence applies.
	const objectBody = async <T>(): Promise<T> => ({}) as T;

	// Typed binding — the annotation `<T>` is a type position and stays bare in
	// BOTH tools; only the initializer `<T>` (value position) takes `<T,>`.
	const typed: <T>() => Promise<T> = async <T>() => ({}) as T;
</script>
