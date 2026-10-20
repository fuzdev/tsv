<script lang="ts">
	// A call value whose first argument is led by a `//` is no break point of its own: when the
	// line up to `(` is over width, the `=` breaks

	// Declarator, `(` line at 100
	const v1aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = useState<T | null>(
		// c
		null
	);

	// Declarator, `(` line at 101
	const v2aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = useState<
		T | null
	>(
		// c
		null
	);

	// Exported declarator, `(` line at 100
	export const v3aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = useState<T | null>(
		// c
		null
	);

	// Exported declarator, `(` line at 101
	export const v4aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = useState<
		T | null
	>(
		// c
		null
	);

	// Member callee, `(` line at 100
	const v5aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = obj.f(
		// c
		null
	);

	// Member callee, `(` line at 101
	const v6aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = obj.f(
		// c
		null
	);

	// Optional call, `(` line at 100
	const v7aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = f?.(
		// c
		null
	);

	// Optional call, `(` line at 101
	const v8aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = f?.(
		// c
		null
	);

	// Assignment, `(` line at 101
	v9aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa =
		useState<T | null>(
			// c
			null
		);

	// A `//` in the member gap: the chain breaks there, the `=` hugs
	const x1aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = aaaa.bbbbbbb // c
		.cccc();

	// A trailing `//` after the last argument, `(` line at 100: the `=` hugs
	const x2aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = f(
		aaaa,
		bbbb // c
	);
</script>
