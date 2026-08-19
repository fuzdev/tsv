<script lang="ts">
	// A block comment the author wrote inside a frozen body's parens stays inside them —
	// the same answer the unfrozen arrow body gives that gap.
	const aaa = () =>
		// prettier-ignore
		(bbb  +  ccc /* c */);

	// A line comment in the same gap opens the pair — inline it would swallow the `)`.
	const ddd = () =>
		// prettier-ignore
		(
			eee  +  fff // c
		);

	// An object body's parens are required, and the comment stays inside them too.
	const ggg = () =>
		// prettier-ignore
		({ hhh:  1 } /* c */);
</script>
