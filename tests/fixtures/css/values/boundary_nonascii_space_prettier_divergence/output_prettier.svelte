<style>
	a {
		/* Single string value led by a non-breaking space (U+00A0) — kept glued. */
		content:   'z';
		/* Single ident value led by a non-breaking space — kept glued (matches prettier). */
		font-family:  q;
		/* Single string value trailed by a non-breaking space. */
		quotes: 'a'  ;
		/* Single ident value trailed by a non-breaking space (matches prettier). */
		font-family: q ;
		/* Single function value trailed by a non-breaking space — kept glued. */
		width: calc(1px)  ;
		/* Single function value led AND trailed by one — both kept glued. */
		height:  calc(1px)  ;
		/* Single string value led by an em space (U+2003). */
		content:   'z';
	}
</style>
