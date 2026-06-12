<script lang="ts">
	// Exactly 100 chars with chain - stays on one line
	const a000000000000000000000000 = obj000000000000000000000000000000.fn000000000(arg).chain00(arg);

	// Exactly 101 chars with chain - chain breaks
	const a0000000000000000000000000 = obj000000000000000000000000000000
		.fn000000000(arg)
		.chain00(arg);

	// Exactly 100 chars - single call (no chain)
	const a0000000000000000000000000000000000000000000000000000000000000000000000000000 = obj.fn(arg);

	// Exactly 101 chars - single call breaks after =
	const a00000000000000000000000000000000000000000000000000000000000000000000000000000 =
		obj.fn(arg);

	// Real-world example: chain fits after break after =
	export const Aaaaaaaaaaaaaaaaaaaaa =
		Bbbbbbbbbbbbbbbbbbbbb.pipe(Ccccccccccccc).brand('Aaaaaaaaaaaaaaaaaaaaa');

	// Very long chain - breaks the chain itself
	const c000000000000000000 = obj
		.method1(argument1)
		.method2(argument2)
		.method3(argument3)
		.method4();

	// Short chains - fit on one line
	const a = obj.fn1().fn2();
	const b = obj.fn(arg).chain();
</script>
