<script lang="ts">
	// Anonymous function expression
	let empty = function () {};
	let noParams = function () {
		return 1;
	};

	// Named function expression
	let named = function myFunc() {
		return 2;
	};
	let namedWithParams = function add(a, b) {
		return a + b;
	};

	// Generator function expression
	let gen = function* () {
		yield 1;
		yield 2;
	};
	let namedGen = function* myGenerator() {
		yield 1;
	};

	// Function expression in assignment
	x = function () {};
	y = function named() {
		return 1;
	};

	// Function expression with parameters and return type
	let typed = function (x: number): number {
		return x * 2;
	};

	// Nested function expressions
	let outer = function () {
		let inner = function () {
			return 1;
		};
		return inner();
	};

	// Function expression as callback
	arr.map(function (x) {
		return x * 2;
	});

	// IIFE (Immediately Invoked Function Expression)
	(function () {
		console.log('iife');
	})();
</script>
