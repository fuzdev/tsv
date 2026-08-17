<script>
	// exactly 100 - the whole call fits, so the deps array stays inline
	new A(() => fn(a), [b, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]);

	// 101 - a two-argument arrow-then-array call declines the hug, every argument breaks out
	new A(
		() => fn(a),
		[b, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]
	);

	// control: the plain call declines the same shape identically
	fn1(
		() => fn(a),
		[b, aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa]
	);

	// control: a third argument is not the shape, so the array still hugs
	new A(b, () => fn(a), [
		c,
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	]);

	// control: a non-arrow first argument is not the shape, so the array still hugs
	new A(b, [
		c,
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	]);
</script>
