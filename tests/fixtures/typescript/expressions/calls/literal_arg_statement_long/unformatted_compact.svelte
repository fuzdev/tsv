<script>
	// A call's lone literal argument breaks like any other argument once the call runs past
	// print width. The <=25-char short-literal threshold is the ASSIGNMENT layout's
	// (calls/string_arg_long); with no operator to break at, the call breaks its own argument.

	// Boundary: exactly 100 chars - fits, stays on one line.
	functionNameAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA('short');

	// Boundary: exactly 101 chars - over width, the argument breaks onto its own line.
	functionNameAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA('short');

	// A numeric literal takes the same path.
	functionNameAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA(12345);
</script>
