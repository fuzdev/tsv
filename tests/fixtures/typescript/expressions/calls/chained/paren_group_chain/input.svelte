<script lang="ts">
	// Chain on parenthesized expression - no spurious blank line
	const a = fn({a: 1, b: 2}).method1(arg).method2(arg);

	// Deeper nesting with await
	const b = (await fn({a: 1, b: 2})).method1(arg).method2(arg);

	// Long args force multiline inside call
	const c = fn({
		a: 1,
		b: 2,
	})
		.method1(arg)
		.method2(arg);
</script>
