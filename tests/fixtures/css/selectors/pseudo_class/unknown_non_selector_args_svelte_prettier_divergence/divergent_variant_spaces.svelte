<style>
	/* a non-selector argument of an unknown functional pseudo-class is kept */
	a:foo(1 + 2) {
		color: red;
	}

	/* glued operators stay glued: spacing them would change the tokens */
	a:foo(1 + 2 1 > 2 1 ~ 2) {
		color: red;
	}

	/* a pseudo-element takes the same argument */
	a::foo(1 + 2) {
		color: red;
	}

	/* strings, commas, nested blocks and functions */
	a:foo('x', 1 + 2) {
		color: red;
	}

	a:foo( ( 1 + 2 )  *  3  [1]  {2} ) {
		color: red;
	}

	a:foo(f( 1  2 ) g( )) {
		color: red;
	}

	/* a top-level semicolon and bang are argument content */
	a:foo( 1;2  ! ) {
		color: red;
	}

	/* a comment inside the argument is kept */
	a:foo( 1  /* c */  2 ) {
		color: red;
	}

	/* numbers, units and hash tokens are kept as written */
	a:foo(.50em + #FFF 1.0 a\ .5) {
		color: red;
	}

	/* a non-ASCII space is an identifier code point, so the argument is not empty */
	a:foo( ) {
		color: red;
	}

	/* the selector continues after the argument */
	a:foo(1 + 2):hover b {
		color: red;
	}

	@supports selector(a:foo(1 + 2)) {
		a {
			color: red;
		}
	}
</style>
