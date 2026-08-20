<script>
	// a `//` the author glued to `=>` keeps that line; the body drops to the
	// continuation below it, the same hang the uncommented arm gives
	const a = (e) => // c
		e.prop;

	const b = () => // c
		g();

	// the rule is the gap's, not the body's — an object, block or ternary body
	// reads the same
	const c = () => // c
		({ a: 1 });

	const d = () => // c
		{
			go();
		};

	const e2 = (x) => // c
		x ? a : b;

	// the same gap inside a call argument, at both argument layouts
	fn(
		a,
		(e) => // c
			e.prop
	);

	fn(a, (e) => // c
		({ b: 1 })
	);

	// a curried chain's head gap answers the same way
	const h =
		(x) => // c
		(y) =>
			z;

	// a run the author wrote together stays together on the `=>` line
	const m = () => /* lead1 */ // lead2
		x;

	// control: an own-line authoring keeps its own line — the two positions are
	// both stable, and the author picks between them
	const n = () =>
		// c
		x;
</script>
