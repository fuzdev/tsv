<script lang="ts">
	// An authored pair around a decorated class at the leftmost position of a `yield` operand
	// is kept: without it the `@` follows `yield`, which acorn (Svelte's parser) rejects
	function* gen() {
		yield @dec
		class {};
		const a = yield @dec
		class {} % 2;
		const b = yield @dec
		class {} ** 2;
		const c = yield @dec
		class {}.x;
		const d = yield @a
		@b
		class {}();
		f(
			yield @dec
			class {}
		);

		// a pair the operand prints anyway is the class's: an assignment's, a sequence's
		const e = yield (@dec
		class {}.x = 1);
		const g = yield (@dec
		class {},
		1);

		// `yield*` takes the class bare in both tools: acorn reads the `@` after the `*`
		yield* @dec
		class {};
	}
</script>
