<style>
	/* a comment inside a feature expression is the node's own text, so neither half
	pads it: a comment glued to the expression's own paren, to a name, or to a value
	stays glued */
	@media (/* c */a: b) and (c: d/* e */) and (f/* g */ h: i) {
		a {
			color: red;
		}
	}
	/* the name half collapses its whitespace runs to one space and the value half
	keeps them, on both sides of a comment */
	@media (a /* c */ b: d) and (e: f  /* g */  h) {
		a {
			color: red;
		}
	}
	/* an inner paren is node text too, and an expression holding nothing but a comment
	keeps its parens tight */
	@media (not (/* c */ a: b)) and (/* c */) {
		a {
			color: red;
		}
	}
</style>
