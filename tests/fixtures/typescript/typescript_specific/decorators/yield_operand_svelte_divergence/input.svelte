<script lang="ts">
	// `yield` takes a decorated class expression as its operand, as tsc and the decorators
	// grammar read it; acorn rejects the `@` after `yield`
	function* gen() {
		yield @dec
		class {};
		const a = yield @dec()
		class A extends B {};
		const b = yield @a.b
		@c
		class {} % 2;
		const c = yield @dec
		class {}.x;
		const d = yield @dec
		class {}();
		const e = yield (@dec
		class {}.x = 1);
		f(
			yield @dec
			class {}
		);
	}
</script>
