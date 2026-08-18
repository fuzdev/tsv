<script lang="ts">
	async function fn1() {
		// The continuation costs one indent level, so the argument list is measured two
		// columns further in than prettier measures it. This one is exactly 100 and stays
		// flat.
		new // c1
			Foo(argument1111111111, argument2222222222, argument3333333333, argument4444444444, arg55555);

		// One character more is 101, so the arguments break — and they break at the
		// continuation's own indent, which is what riding the continuation with the whole
		// tail (rather than the callee alone) buys.
		new // c2
			Foo(
				argument1111111111,
				argument2222222222,
				argument3333333333,
				argument4444444444,
				arg555555
			);

		// The `await` operand answers the same way.
		await // c3
			obj.method(
				argument1111111111,
				argument2222222222,
				argument3333333333,
				argument44444444444444
			);
	}
</script>
