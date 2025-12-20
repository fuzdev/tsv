<script lang="ts">
	// Empty arrow body - stays hugged
	fn(() => {});
	fn1(() => {});

	// Arrow with body content - expands
	fn(() => {
		return 1;
	});

	// Expression arrow - stays inline
	fn((x) => x + 1);
	arr.map((x) => x * 2);
	arr.filter((x) => x > 0);

	// Typed parameters in callback
	fn((x: number) => x + 1);
	fn((x: number, y: string) => x);

	// Arrow with statements - expands
	arr.forEach((x) => {
		fn(x);
	});

	// Multiple callbacks
	promise.then(
		(x) => {
			fn1(x);
		},
		(y) => {
			fn2(y);
		},
	);

	// Callback with other args
	fn1(() => {
		fn2();
	}, 100);

	// Async callbacks
	fn(async () => {
		await fn1();
	});

	// Generic callbacks (trailing comma required for single type param in TSX-compatible syntax)
	fn(<T,>(x: T) => x);
	fn(<T, U>(x: T, y: U) => x);

	// Callback returning object literal (needs parens)
	arr.map((x) => ({a: x}));

	// Arrow with explicit return type
	fn((): number => 1);
	fn((x: number): string => String(x));

	// Nested callbacks (inline)
	fn(() => fn(() => {}));

	// Callback in generic call
	fn<T>((x) => x);
</script>
