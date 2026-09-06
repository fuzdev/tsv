<style>
	/* an escaped quote is ident content, not a string delimiter, so the real string
	beside it still takes prettier's quote normalization */
	@supports (a: x\"y 'z') {
		b {
			color: red;
		}
	}
</style>
