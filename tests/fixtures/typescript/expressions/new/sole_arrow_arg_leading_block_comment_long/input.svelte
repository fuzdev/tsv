<script lang="ts">
	// A lone arrow argument led by a block comment stays on the `new` line when it fits —
	// object, array and object-terminal arrow-chain bodies, at every statement position

	// Statement, object body
	new Fn(/* l */ (x) => ({ k: a }));

	// Array body
	new Fn(/* l */ (x) => [a]);

	// Arrow chain with an object terminal
	new Fn(/* l */ (x) => (y) => ({ k: a }));

	// Type arguments
	new Fn<T>(/* l */ (x) => ({ k: a }));

	// Member callee
	new a.Fn(/* l */ (x) => ({ k: a }));

	// JSDoc lead
	new Fn(/** l */ (x) => ({ k: a }));

	// `return` and `throw`
	function fn1() {
		return new Fn(/* l */ (x) => ({ k: a }));
	}
	function fn2() {
		throw new Fn(/* l */ (x) => ({ k: a }));
	}

	// `export default`
	export default new Fn(/* l */ (x) => ({ k: a }));

	// A declarator value: the same flat form
	const b = new Fn(/* l */ (x) => ({ k: a }));

	// Block body: the argument breaks out
	new Fn(
		/* l */ (x) => {
			return a;
		}
	);

	// Ternary body
	new Fn(/* l */ (x) => (c ? a : b));

	// A plain call
	fn(/* l */ (x) => ({ k: a }));

	// Exactly 100 columns: stays flat
	new Fn(/* l */ (x) => ({ k: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa }));

	// 101 columns: the argument breaks out
	new Fn(
		/* l */ (x) => ({ k: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaab })
	);
</script>

{new Fn(/* l */ (x) => ({ k: a }))}
