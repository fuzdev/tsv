<script lang="ts">
	// A block comment the author wrote inside a frozen initializer's clarity parens stays
	// inside them: the pair is the printer's, so the comment would have to cross a `)` the
	// output still prints to reach the statement's `;`.
	const aaa =
		// prettier-ignore
		((bbb  =  ccc /* c */));

	// A line comment in the same gap opens the pair — inline it would swallow the `)`.
	const ddd =
		// prettier-ignore
		(
			(eee  =  fff // c
			)
		);

	// A frozen sequence supplies its own required parens, and that gap sits inside them
	// the same way.
	const ggg =
		// prettier-ignore
		((hhh,  iii /* c */));
</script>
