<script>
	// Call expression - dangling comment in empty args
	fn(/* dangling */);

	// New expression - dangling comment in empty args
	new A(/* dangling */);

	// Arrow function - dangling comment in empty params
	let a = (/* dangling */) => {};

	// Function declaration - dangling comment in empty params
	function b(/* dangling */) {}

	// Function expression - dangling comment in empty params
	let c = function (/* dangling */) {};

	// Method - dangling comment in empty params
	let d = { fn(/* dangling */) {} };

	// Optional call - dangling comment
	fn?.(/* dangling */);

	// Comment containing close paren character
	function e(/* has ) paren */) {}
	let f = (/* has ) paren */) => {};
</script>
