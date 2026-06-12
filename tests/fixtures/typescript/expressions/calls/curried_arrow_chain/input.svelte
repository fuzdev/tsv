<script>
	// Short curried arrow - stays inline
	fn(() => (x) => x);

	// Curried arrow with destructuring param as call arg - hugs body
	fn(() => ({a}) => {
		return a;
	});

	// Curried arrow with block body
	fn(() => (x) => {
		return x;
	});
</script>
