<script>
	// Short curried chain - stays fully inline
	const short = (a) => (b) => (c) => a + b + c;

	// Boundary: heads + object opening fit on the `=` line (exactly 100), object body breaks
	const boundaryFits = (argument1) => (argument2) => (argument3) => (argument4) => (argument5) => ({
		foo: argument1
	});

	// Boundary: one char over (101) breaks after `=`; heads still fit together on one line
	const boundaryBreak =
		(argument1) => (argument2) => (argument3) => (argument4) => (argument55) => ({
			foo: argument1
		});

	// Heads do not fit together either - each arrow head drops to its own line
	const eachHead =
		(argument1) =>
		(argument2) =>
		(argument3) =>
		(argument4) =>
		(argument5) =>
		(argument6) =>
		(argument7) =>
		(argument8) => ({ foo: argument1, bar: argument2 });

	// Each head on its own line with a block body terminal
	const blockBody =
		(argument1) =>
		(argument2) =>
		(argument3) =>
		(argument4) =>
		(argument5) =>
		(argument6) =>
		(argument7) =>
		(argument8) => {
			return argument1;
		};
</script>
