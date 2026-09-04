<style>
	a {
		/* An unquoted url() argument glued to a no-break space (U+00A0): the space is not
		   CSS whitespace, so it is url content, kept at either end */
		background: url(  a );
		background: url( a  );
		background: url(  a  );
		/* A quoted argument glued to one is a single opaque token, kept verbatim */
		background: url(  "a" );
	}
</style>
