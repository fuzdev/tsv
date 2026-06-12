<script>
	const id = (x) => x;
	const double = (n) => n * 2;
	const greet = (name) => `Hello ${name}`;
	const nested = (a) => (b) => a + b;
	const withBody = (x) => {
		return x + 1;
	};
	const asyncSingle = async (x) => x;
	const asyncBody = async (x) => {
		return await x;
	};
</script>
