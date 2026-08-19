<script lang="ts">
	// A block comment the author wrote inside a frozen init declarator's clarity parens stays
	// inside them: the pair is the printer's, so the comment would have to cross a `)` the
	// output still prints to reach the clause's `;`.
	for (
		let aaa =
			// prettier-ignore
			(bbb  =  ccc /* c */);
		aaa;
		aaa
	) {
		fn();
	}

	// A line comment in the same gap opens the pair — inline it would swallow the `)`.
	for (
		let ddd =
			// prettier-ignore
			(
				eee  =  fff // c
			);
		ddd;
		ddd
	) {
		fn();
	}

	// A frozen sequence supplies its own required parens, and that gap sits inside them the
	// same way.
	for (
		let ggg =
			// prettier-ignore
			(hhh,  iii /* c */);
		ggg;
		ggg
	) {
		fn();
	}
</script>
