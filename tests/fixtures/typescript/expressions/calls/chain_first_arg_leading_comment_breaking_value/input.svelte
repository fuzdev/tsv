<script lang="ts">
	// The chain's argument printer shares the first-argument rule: a run glued to
	// `(` that the author broke after takes its own line when the argument breaks.
	obj.m1().m2(
		/* c1 */
		{
			b: 1,
			c: 2
		}
	);

	// The broken chain's argument printer too.
	obj2
		.m3aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa()
		.m4bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb()
		.m5ccccccccccccccccccccccccccccccc(
			/* c2 */
			{
				b: 1,
				c: 2
			}
		);
</script>
