<style>
	/* the prelude ends in an escape whose payload IS that space (the layer is named
	   `a `), so trimming it strands the backslash onto the `;` */
	@layer a\;

	/* the block form: the payload space already separates the prelude from the `{`, so
	   no second separator is added */
	@layer b\ {
		div {
			color: red;
		}
	}
</style>
