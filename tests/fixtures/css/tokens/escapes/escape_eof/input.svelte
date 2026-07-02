<style>
	div {
		/* complete escape ending a string value */
		content: '\61';
		/* complete escape ending an unquoted ident value */
		grid-area: \61;
		/* complete escape inside url() */
		background: url(x\61);
	}
</style>
