<script>
	// A curried chain in a parameter default resolves on its own: the enclosing chain's
	// heads break (the default forces it) and its terminal body hangs one level under the
	// last head, however the chain in the default laid out

	// Call argument, with a plain and a destructured default and a computed key
	fn((a = (x) => (y) => z) => (b) => c);
	fn(({ a = (x) => (y) => z }) => (b) => c);
	fn(({ [(x) => (y) => z]: a }) => (b) => c);

	// Logical operand and return argument
	a = b || ((c = (x) => (y) => z) => (d) => e);
	function fn1() { return (a = (x) => (y) => z) => (b) => c; }

	// A chain nested two defaults deep: each level's body hangs under its own heads
	fn((a = (b = (x) => (y) => z) => (c) => d) => (e) => f);
</script>
