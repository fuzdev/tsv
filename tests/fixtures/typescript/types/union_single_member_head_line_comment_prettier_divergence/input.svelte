<script lang="ts">
	// A `//` after a sole member's authored pipe is the enclosing seam's, and takes
	// the seam's continuation exactly as the pipe-less authoring does
	let a: // c
		{ a: number };
	type R = () => // c
		A;
	const b = x as // c
		A;
	type D = T extends // c
		A
		? 1
		: 2;
	type F<
		T = // c
			A
	> = T;
	function g(x: unknown): x is // c
		A {}
	type H = keyof // c
		A;
	type J = T extends U
		? // c
			A
		: 2;
	type K = {
		[
			P in // c
			A
		]: 1;
	};
	interface L {
		p: // c
			{ a: number };
	}
	type N =
		X[// c
		K];
	type M = // c
		{ a: number };
	type Q = Foo< // c
		A
	>;
</script>
