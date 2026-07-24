<script lang="ts">
	// an own-line directive between members freezes only the following member;
	// the tuple stays broken one member per line
	type A = [
		a,
		// prettier-ignore
		{x:   1},
		b
	];

	// before the first member: freezes that member (Rule A — first member like every member)
	type B = [
		// prettier-ignore
		{x:   1},
		b
	];

	// an own-line block comment behaves identically — placement keys the freeze, not the spelling
	type C = [
		/* prettier-ignore */
		{x:   1},
		b
	];

	// single member
	type D = [
		// prettier-ignore
		{x:   1}
	];

	// a multi-line frozen member keeps its verbatim layout; the `,` is parent-owned
	type E = [
		a,
		// prettier-ignore
		{
			x:   1,
				y: 2
		},
		b
	];
</script>
