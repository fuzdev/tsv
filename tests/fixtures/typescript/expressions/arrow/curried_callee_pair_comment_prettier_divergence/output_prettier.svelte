<script lang="ts">
	// A comment inside the pair around a curried-chain callee keeps the pair closed against the
	// chain: the leading block leads the first head, the trailing one trails the body.
	(
		/* c */ (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
			(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
			(cccccccccccccccccccccccccccccccccc) =>
				ddddddddd

	)();
	(
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
			(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
			(cccccccccccccccccccccccccccccccccc) =>
				ddddddddd
		 /* t */
	)();

	// null control: with no comment the pair opens onto its own lines, in both formatters
	(
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
		(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
		(cccccccccccccccccccccccccccccccccc) =>
			ddddddddd
	)();
</script>
