<script lang="ts">
	// Simple assignment
	const fn1 = () => {
		return (a = b);
	};

	// Nested assignment
	const fn2 = () => {
		return (a = b = c);
	};

	// Assignment with operator
	const fn3 = () => {
		return (a = b - c);
	};

	// Compound assignment
	const fn4 = () => {
		return (a += b);
	};

	// Assignment in expression
	const fn5 = () => {
		return (s2 = t - (c = t | 0));
	};

	// Non-assignment (no parens needed)
	const fn6 = () => {
		return a + b;
	};

	// Logical expression in parens that fits on one line
	const fn7 = () => {
		return (
			(a as RegExp).source === (b as RegExp).source && (a as RegExp).flags === (b as RegExp).flags
		);
	};

	// Logical expression that needs to break (over 100 chars)
	const fn8 = () => {
		return (
			(aaaaaaaaaa as RegExp).source === (bbbbbbbbbb as RegExp).source &&
			(aaaaaaaaaa as RegExp).flags === (bbbbbbbbbb as RegExp).flags
		);
	};
</script>
