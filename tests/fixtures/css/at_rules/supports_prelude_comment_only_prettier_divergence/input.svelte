<style>
	/* a condition prelude holding only comments still emits them — prettier drops the
	last byte of the closing delimiter, leaving an unterminated comment its own parser
	then rejects */
	@supports /* c */;
	div {
		color: red;
	}
</style>
