<style>
	/* A comment splits the An+B term: inter-token trivia per CSS Syntax 3, so the term
	   still reads as one Nth and the gaps around the comment normalize to one space. */
	a:nth-child(2n /* c1 */ + 1) {
		color: red;
	}

	/* the term's other interior gap: between the sign and the signless integer */
	a:nth-child(2n + /* c2 */ 1) {
		color: blue;
	}

	/* a run of comments in one gap */
	a:nth-child(2n/* c3 *//* c4 */+1)
	{
		color: green;
	}

	/* composes with the spec-only negative forms */
	a:nth-child(-2n /* c5 */ - 3) {
		color: teal;
	}

	/* a bare n also has a type-selector reading, so this is the term the selector-list
	   fallback would silently accept instead - it must read as one Nth like the rest */
	a:nth-child(n /* c6 */ + 1) {
		color: purple;
	}

	/* operator characters inside the comment content are never respaced */
	a:nth-child(2n /* a+b */ + 1) {
		color: orange;
	}

	/* composes with of S */
	a:nth-child(2n /* c7 */ + 1 of .class) {
		color: navy;
	}

	/* even and odd take no tail, so this is not an An+B term at all: it stays demoted to
	   the selector-list path (a type selector, a combinator and a bare Nth) */
	a:nth-child(even /* c8 */ + 1) {
		color: olive;
	}
</style>
