<style>
	div {
		/* an unspaced operator run travels whole; a var() no line holds breaks in place */

		/* 100 chars - the run fits its line */
		width: calc(100%-var(--jjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjj));

		/* 101 chars - the var() opens its own parens, glued to the operator */
		width: calc(100%-var(--jjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjjj));
	}
</style>
