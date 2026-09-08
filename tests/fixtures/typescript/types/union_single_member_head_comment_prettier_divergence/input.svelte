<script lang="ts">
	// An own-line block after a sole member's authored pipe compacts as the bare
	// annotation's does, at every seam whose bare rule compacts it
	let a: /* c */ { a: number };
	type R = () => /* c */ { a: number };
	const b = x as /* c */ { a: number };
	type D = T extends /* c */ { a: number } ? 1 : 2;
	type F<T extends /* c */ { a: number }> = T;
	function g(x: unknown): x is /* c */ { a: number } {}
	type H = keyof /* c */ { a: number };
	type J = T extends U ? /* c */ { a: number } : 2;
	type K = { [P in /* c */ A]: 1 };
	interface L {
		p: /* c */ { a: number };
	}
	type N = X[/* c */ K];
	let o: /* c */ A;

	// At `=` the bare own-line block keeps its line, and the pipe authoring reaches it
	type E =
		/* c */
		{ a: number };
</script>
