<script lang="ts">
	// 100 chars - fits on one line
	const a = fn({ a: 1 }, (x) => cb(x, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10));

	// 101 chars - breaks: preceding args inline, body after =>
	const b = fn({ a: 1 }, (x) =>
		cb(x, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10a)
	);

	// Multiple preceding args - all stay inline when body breaks (101 chars)
	const c = fn('a', { b: 1 }, (x) =>
		cb(x, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9abc)
	);
</script>
