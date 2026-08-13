<script>
	// Boundary: heads + the object opening fit on the `=` line (exactly 100) and stay there;
	// a comment inside the body does not move them
	const boundaryFits = (argument1) => (argument2) => (argument3) => (argument4) => (argument5) => ({
		foo: argument1 // c
	});

	// Boundary: one char over (101) breaks after `=`; the heads still fit together on one line
	const boundaryBreak =
		(argument1) => (argument2) => (argument3) => (argument4) => (argument55) => ({
			foo: argument1 // c
		});

	// Heads do not fit together either - each head drops to its own line, body comment or not
	const eachHead =
		(argument1) =>
		(argument2) =>
		(argument3) =>
		(argument4) =>
		(argument5) =>
		(argument6) =>
		(argument7) =>
		(argument8) => ({ foo: argument1 /* c */ });

	// A block-statement terminal reads the same way
	const blockBody =
		(argument1) =>
		(argument2) =>
		(argument3) =>
		(argument4) =>
		(argument5) =>
		(argument6) =>
		(argument7) =>
		(argument8) => {
			return argument1; // c
		};

	// Control: no comment at all
	const plain =
		(argument1) =>
		(argument2) =>
		(argument3) =>
		(argument4) =>
		(argument5) =>
		(argument6) =>
		(argument7) =>
		(argument8) => ({ foo: argument1 });
</script>
