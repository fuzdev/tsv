<script>
	// `async [no LineTerminator here] function`: across a line break `async` is a plain
	// identifier, so this is `new async` and a function declaration, not an async
	// function expression
	const x = new async(); // c
	function f1() {
		return 1;
	}
</script>
