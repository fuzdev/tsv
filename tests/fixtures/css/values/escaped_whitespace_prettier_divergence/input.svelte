<style>
	div {
		width: 50px\ ;
		height: var(--a, 50px\ );
		margin: calc(1px\ );
		top: calc\ (1px);
		color: a\ b;
		padding: var(--b, x\,y);
		gap: a, b\,;
		inset: a\+b;
		/* the whitespace normalizer's own delimiter strips reach the payload too: a value
		the comment path re-emits from source, and a paren shell the value parser left opaque */
		left: (1px\ ) /* c */;
		right: f(1px\ , 2px) /* c */;
		bottom: (a b c\ )(d);
	}
</style>
