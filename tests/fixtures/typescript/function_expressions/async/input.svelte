<script lang="ts">
	// Anonymous async function expression
	let empty = async function () {};
	let noParams = async function () {
		return 1;
	};

	// Named async function expression
	let named = async function myFunc() {
		return 2;
	};
	let namedWithParams = async function add(a, b) {
		return a + b;
	};

	// Async function expression with await
	let withAwait = async function () {
		const result = await fetch(url);
		return result.json();
	};

	// Async function expression in assignment
	x = async function () {};
	y = async function named() {
		return 1;
	};

	// Async function expression with parameters and return type
	let typed = async function (x: number): Promise<number> {
		return x * 2;
	};

	// Nested async function expressions
	let outer = async function () {
		let inner = async function () {
			return 1;
		};
		return await inner();
	};

	// Async function expression as callback
	arr.map(async function (x) {
		return await process(x);
	});

	// Async IIFE
	(async function () {
		await doSomething();
	})();
</script>
