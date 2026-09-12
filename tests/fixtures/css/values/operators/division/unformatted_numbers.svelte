<style>
	div {
		/* a `/` between two numbers is an operator: both sides normalize and the
		authored gap is kept exactly as written */
		margin: 1.50/2.50;
		padding: 1.50 / 2.50;
		width: 1.50 /2.50;
		height: 1.50/ 2.50;
		/* a run that starts with an ident character absorbs the `/`, so it is one
		token and nothing inside it normalizes */
		top: a/1.50;
		left: #FFF/1.50;
		/* a word or a function on either side of the operator spaces it */
		right: 1.50 / aa;
		bottom: 1.50 / f(2.50);
		flex: f(2.50) / 1.50;
		/* the rule is per pair, so one chain can be glued at one operator and
		spaced at the next */
		border-width: 1.50/2.50 / aa;
		/* a unit ends at the operator */
		outline-width: 1.50px/2.50;
		/* a leading `/` takes no space after it */
		column-gap: /1.50;
		/* the `font` shorthand and a custom property keep a glued `/` beside a
		number, a var(), a calc(), a min(), a max() or a clamp() */
		font: 1.50/aa;
		--a: 1.50/aa;
	}
</style>
