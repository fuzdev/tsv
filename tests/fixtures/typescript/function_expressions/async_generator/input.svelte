<script lang="ts">
	// Anonymous async generator function expression
	let empty = async function* () {};
	let basic = async function* () {
		yield 1;
		await delay(100);
		yield 2;
	};

	// Named async generator function expression
	let named = async function* myGen() {
		yield 1;
	};
	let namedWithParams = async function* gen(a, b) {
		yield a;
		yield b;
	};

	// Async generator with await and yield
	let combined = async function* () {
		const data = await fetch(url);
		yield data;
		yield* otherGen();
	};

	// Async generator expression in assignment
	x = async function* () {};
	y = async function* named() {
		yield 1;
	};

	// Async generator with parameters and return type
	let typed = async function* (x: number): AsyncGenerator<number> {
		yield x;
	};

	// For await inside async generator
	let withForAwait = async function* () {
		for await (const item of asyncIterable) {
			yield item;
		}
	};
</script>
