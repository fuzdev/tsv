<script lang="ts">
	// Arrow body: object as chained type assertions
	// Single as - already works (parens on object)
	const a = (): Logger => ({error: vi.fn(), warn: vi.fn()}) as Logger;

	// Chained as unknown as T - the bug case
	const b = (): Logger =>
		({
			error: vi.fn(),
			warn: vi.fn(),
			info: vi.fn(),
			debug: vi.fn(),
			raw: vi.fn(),
		}) as unknown as Logger;

	// Short chained as that fits on one line
	const c = (): T => ({a: 1}) as unknown as T;

	// satisfies + as chain
	const d = () => ({a: 1}) satisfies A as B;

	// Single satisfies (should already work like single as)
	const e = () => ({a: 1}) satisfies A;

	// Triple chain
	const f = (): T => ({x: 1}) as unknown as Partial<T> as T;
</script>
