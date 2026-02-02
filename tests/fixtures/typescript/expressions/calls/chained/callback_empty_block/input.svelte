<script>
	// Empty block callbacks - chain stays inline (2 callbacks)
	arr.map(() => {}).filter(() => {});

	// 3+ empty block callbacks - Prettier breaks at 3+
	a.b(() => {})
		.c(() => {})
		.d(() => {});

	// Empty blocks with parameters
	arr.map((x) => {}).filter((y) => {});

	// Multi-arg calls with empty block callbacks (original failing case)
	bench.add('task 1', () => {}).add('task 2', () => {});

	// Single call with empty block
	arr.filter(() => {});

	// Mixed: empty and non-empty - breaks because non-empty breaks
	arr
		.map(() => {
			x;
		})
		.filter(() => {});

	// Function expressions with empty blocks - also inline
	arr.map(function () {}).filter(function () {});

	// Function expressions with parameters
	arr.map(function (x) {}).filter(function (y) {});

	// Mixed function expression: non-empty breaks the chain
	arr
		.map(function () {
			x;
		})
		.filter(function () {});
</script>
