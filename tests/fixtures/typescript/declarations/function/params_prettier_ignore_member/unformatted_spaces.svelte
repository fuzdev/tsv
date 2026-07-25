<script lang="ts">
	// an own-line directive in a parameter gap freezes only the following parameter;
	// the list stays broken one parameter per line
	function a(
		p:   T  ,
		// prettier-ignore
		q:   {x:   1},
			r:   U
	) {}

	// before the first parameter: freezes that parameter (Rule A — first like every)
	function b(
		// prettier-ignore
		p:   {x:   1},
		q:   U
	) {}

	// an own-line block comment behaves identically — placement keys the freeze, not the spelling
	function c(
		/* prettier-ignore */
		p:   {x:   1},
			q:   U
	) {}

	// arrow parameters
	const d = (
		// prettier-ignore
		p:   {x:   1},
		q:   U
	) => p;

	// a class method and an object method share the same parameter printer
	class E {
		m(
			// prettier-ignore
			p:   {x:   1},
			q:   U
		) {}
	}

	const f = {
		m(
			// prettier-ignore
			p:   {x:   1},
				q:   U
		) {}
	};

	// a parameter property freezes with its modifier; a decorator rides inside the slice
	class G {
		constructor(
			// prettier-ignore
			public p   = 1,
			// prettier-ignore
			@dec(  2 ) q: T,
			r:   U
		) {}
	}

	// a rest parameter freezes from its `...`
	function h(
		// prettier-ignore
		...  rest: U[]
	) {}

	// a lone huggable parameter expands instead — a hug would pull the directive off its line
	function i(
		// prettier-ignore
		{ a,   b }: T
	) {}

	// an optional marker rides inside the frozen slice
	function j(
		// prettier-ignore
		p?:   {x:   1},
		q:   U
	) {}

	// a multi-line frozen parameter keeps its verbatim layout; the `,` is parent-owned
	function k(
		p:   T  ,
		// prettier-ignore
		q: {
			x:   1,
				y: 2
		},
			r:   U
	) {}
</script>
