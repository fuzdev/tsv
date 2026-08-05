<script lang="ts">
	// Short: cast stays attached, call breaks internally (a block-body arg always breaks it)
	<A>fn1(
		b,
		(y) => {
			fn2();
		},
		c
	);

	// Prefix to the block body exactly 100: cast attached, args expand
	<A>fn3(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		(y) => {
			fn4();
		},
		c
	);

	// Prefix to the block body at 101: same layout — the width up to the arg
	// list's own break never decides the cast, so the call still breaks
	// internally and the cast stays attached
	<A>fn5(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		(y) => {
			fn6();
		},
		c
	);
</script>
