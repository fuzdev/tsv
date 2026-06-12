<script>
	function fn1() {
		// line comment before return value
		return (
			// comment
			b
		);
	}

	function fn2() {
		// line comment in return binary (corpus: css-prune.js)
		return (
			// not after a :global selector
			!a &&
			!(b === 'c' && d === null) &&
			// not a :global(...)
			!e
		);
	}

	function fn3() {
		throw (
			// comment
			expr
		);
	}
</script>
