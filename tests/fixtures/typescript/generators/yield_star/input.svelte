<script>
	// Basic yield*
	function* gen() {
		yield* iterable;
		yield* generator();
	}

	// yield* with array
	function* gen2() {
		yield* [1, 2, 3];
		yield* arr;
	}

	// yield* return value
	function* gen3() {
		const result = yield* otherGen();
		return result;
	}

	// yield* with string (iterates chars)
	function* gen4() {
		yield* 'abc';
	}

	// Combined yield and yield*
	function* gen5() {
		yield 1;
		yield* [2, 3];
		yield 4;
	}
</script>
