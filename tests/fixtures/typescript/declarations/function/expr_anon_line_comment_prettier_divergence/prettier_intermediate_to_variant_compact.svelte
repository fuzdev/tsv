<script>
	// Anonymous function - no params
	const a = function () // c1
	{};

	// Anonymous function - with params
	const b = function (
		// c2
		x,
	) {
		return x;
	};

	// Anonymous generator
	const c = function* () // c3
	{};

	// Async anonymous function
	const d = async function () // c4
	{};

	// Export default anonymous function
	export default function () // c5
	{}
</script>
