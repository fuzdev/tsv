<style>
	/* `@import`'s prelude is value-parsed, like `@supports`: a number normalizes whatever
	its unit, a hex colour lowercases, and a number abutting the token before it is kept */
	@import 'a.css' (a: 1.5abc) and (b: #FFF);
	@import 'b.css' (a: 1a.50) and (b: #1.50);
	/* a single parenthesized condition reaches the same normalization */
	@import 'c.css' (a: 0.5PX);
	@import 'd.css' layer(e) (a: 0.5PX);
</style>
