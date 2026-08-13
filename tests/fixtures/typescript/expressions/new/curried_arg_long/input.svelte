<script>
	// Short curried chain as a new argument - stays fully inline
	new Foo('x', (a) => (b) => (c) => a + b + c);

	// Lone argument: the head signatures fit on one line (exactly 100), only the body drops
	new Foo(
		(argument1) => (argument2) => (argument3) => (argument4) => (argument5) => (argument6xxxxxxx) =>
			body
	);

	// One char over (101): the heads progressive-indent (first head stays, rest indent one level)
	new Foo(
		(argument1) =>
			(argument2) =>
			(argument3) =>
			(argument4) =>
			(argument5) =>
			(argument6xxxxxxxx) =>
				body
	);

	// A preceding argument plus a long first head - the chain still progressive-indents
	new Foo(
		'first',
		(argument1111111, argument2222222, argument3333333, argument4444444, argument55) =>
			(argument6) =>
				body
	);

	// Block body terminal on a lone argument - the block opens on the last head's line
	new Foo(
		(argument1111111, argument2222222, argument3333333, argument4444444, argument5555555) =>
			(argument6) => {
				return body;
			}
	);

	// An authored blank line between arguments is kept; the chain still progressive-indents
	new Foo(
		aaa,

		(argument1111111, argument2222222, argument3333333, argument4444444, argument5555555) =>
			(argument6) =>
				body
	);
</script>
