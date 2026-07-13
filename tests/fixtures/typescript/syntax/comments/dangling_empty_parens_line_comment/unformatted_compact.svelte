<script>
	// Call expression - line comment in empty args
	fn(// dangling
	);

	// New expression - line comment in empty args
	new A(// dangling
	);

	// Optional call - line comment in empty args
	fn?.(// dangling
	);

	// Member chain - line comment in empty args
	obj.method1().method2(// dangling
	);

	// Arrow function - line comment in empty params
	let a = (// dangling
	) => {};

	// Function declaration - line comment in empty params
	function b(// dangling
	) {}

	// Method - line comment in empty params
	let c = {
		fn(// dangling
		) {}
	};

	// Multiple line comments - each on its own line
	fn(

		// dangling1

		// dangling2

	);

	// Block comment then line comment - each on its own line
	fn(/* dangling */ // dangling
	);

	// Comment containing close paren character
	fn(// has ) paren
	);
</script>
