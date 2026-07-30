<style>
	div {
		/* the list ends in an empty element, so it takes one more comma than separators */
		transition:
			a,
			;
		/* the same inside a function */
		background: linear-gradient(red, );
		/* a custom property has no grammar to invalidate: the element is content outright */
		--x: a, ;
	}
</style>
