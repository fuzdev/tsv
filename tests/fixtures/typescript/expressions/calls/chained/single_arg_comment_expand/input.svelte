<script>
	// Arrow with object body: leading comment forces expansion
	const a = b
		.then((x) => x)
		.catch(
			/* comment */ async (e) => ({
				error: await fn(a, b, c, e)
			})
		);

	// Function expression: leading comment forces expansion
	const c = d.method().other(
		/* comment */ function () {
			return 1;
		}
	);

	// Without comment: hugs naturally (contrast)
	const e = f
		.then((x) => x)
		.catch(async (e) => ({
			error: await fn(a, b, c, e)
		}));
</script>
