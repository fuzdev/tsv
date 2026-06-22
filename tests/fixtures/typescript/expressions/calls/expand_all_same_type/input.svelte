<script>
	// two objects (same type) where second is multi-line - expands all
	fn1(() => {
		fn2(async () => {
			await fn3(
				{ a: fn4() },
				{
					b: 'c',
					d,
					e: f
				}
			);
		});
	});

	// two objects where both fit flat - stays inline
	fn1(() => {
		fn2(async () => {
			await fn3({ a: fn4() }, { b: 'c', d, e: f });
		});
	});
</script>
