<script>
	// A curried chain as a callee keeps its parens closed around a hugging or block body
	// that stays on the last head's line, however a chain nested in that body broke

	// Object, array and block bodies, each holding a chain whose default breaks it
	((a) => (b) => ({
		k: fn(
			(x = 1) =>
				(y) =>
					z
		)
	}))();
	((a) => (b) => [
		(x = 1) =>
			(y) =>
				z
	])();
	((a) => (b) => {
		(x = 1) =>
			(y) =>
				z;
	})();
	new ((a) => (b) => [
		(x = 1) =>
			(y) =>
				z
	])();

	// Two chains nested in the body, in either order: one breaks, one stays inline, and
	// neither decides the enclosing chain's `)`
	((a) => (b) => [
		(x = 1) =>
			(y) =>
				z,
		(c) => (d) => e
	])();
	((a) => (b) => [
		(c) => (d) => e,
		(x = 1) =>
			(y) =>
				z
	])();
</script>
