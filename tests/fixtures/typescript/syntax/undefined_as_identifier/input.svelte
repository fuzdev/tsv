<script>
	// `undefined` is a global identifier (not a reserved word) — valid as a
	// reference, an assignment target, and a binding name in every position.
	var undefined;
	undefined = 12;
	let a = undefined;
	undefined;

	// binding name in parameter / destructuring positions (each its own scope)
	function fn(undefined) {}
	function fn2({ undefined }) {}
	function fn3([undefined]) {}
	const arrow = (undefined) => undefined;
</script>
