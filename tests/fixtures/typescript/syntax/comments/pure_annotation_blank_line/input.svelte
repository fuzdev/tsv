<script>
	// A multi-line annotation's own newlines are physically in the source but are not an
	// authored blank line — the node it leads stays adjacent to the one before it.

	// Statement list
	first();
	/* @__PURE__
   keep */ fn();
	last();

	// An authored blank line before the annotation is still preserved
	first();

	/* @__PURE__
   keep */ fn();

	// Block statement list
	function f() {
		first();
		/* @__PURE__
   keep */ fn();
		last();
	}

	// Array elements
	const arr = [
		a,
		/* @__PURE__
   keep */ fn(),
		b
	];

	// Call arguments
	g(
		a,
		/* @__PURE__
   keep */ fn(),
		b
	);

	// Member-chain call arguments
	obj.a().b().g(
		a,
		/* @__PURE__
   keep */ fn(),
		b
	);
</script>
