<script lang="ts">
	// A directive in the `(`→argument gap freezes the template AND expands the call: the
	// hug is a flat concat, so it would land the directive on the `(` line, where it is
	// inert — and the freeze would be gone on the second pass.

	// Plain call
	fn(
		// prettier-ignore
		/* x */ `${a  +  b}
y`
	);

	// `new`
	new Comp(
		// prettier-ignore
		/* x */ `${a  +  b}
y`
	);

	// Member chain
	obj.m(
		// prettier-ignore
		/* x */ `${a  +  b}
y`
	);

	// Dynamic import
	import(
		// prettier-ignore
		/* x */ `${a  +  b}
y`
	);

	// Block spelling of the directive
	fn(
		/* prettier-ignore */
		/* x */ `${a  +  b}
y`
	);

	// A real member chain declines too, and keeps its own broken-head layout — the flat
	// form prettier reaches is the one this call never gets
	a.b()
		.c()
		.d(
			// prettier-ignore
			/* x */ `${a  +  b}
y`
		);

	// Control: with nothing glued to the backtick the newline before it declines the hug
	// on its own, so both tools expand
	fn(
		// prettier-ignore
		`${a  +  b}
y`
	);
</script>
