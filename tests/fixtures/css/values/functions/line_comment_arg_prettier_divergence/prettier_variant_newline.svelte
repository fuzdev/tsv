<style>
	/* CSS has no line comments: a `//` inside a function's parens is two `/` delimiters, and
	the argument list normalizes around it as any other */
	a {
		color: fn(//x, 0.1);
	}
	@supports (a: fn(//x, 0.1)) {
	}
	@import 'a.css' screen fn(//x, 0.1);
	@import ur
		fn(//x, 0.1);
</style>
