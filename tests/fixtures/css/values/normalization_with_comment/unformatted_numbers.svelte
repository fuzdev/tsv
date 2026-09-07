<style>
	div {
		/* a value holding a comment still takes every value-level rule the comment-free
		path takes: a number canonicalizes, a hex colour lowercases, a string re-quotes
		and a unit folds to its canonical case */
		margin: 1.50px /* comment */ 2px;
		padding: .50px /* comment */ 2px;
		border-color: #fff /* comment */;
		font-family: 'a' /* comment */;
		width: 1px /* comment */ 2px;
		line-height: 1.0e3 /* comment */;
		/* the rules reach a function's own argument, and a trailing comment does not
		stop the value before it */
		height: calc(1.50px + 2.50px) /* comment */;
		background-color: rgba(0, 0, 0, 0.50 /* comment */);
		/* a custom property takes the same path */
		--a: 1.50px /* comment */ 2px;
		/* an unquoted url() is opaque, so its content is copied whether or not the
		value carries a comment */
		background-image: url(a1.50.png) /* comment */;
		/* a parenthesized group takes the rules too, with the comment inside it or after */
		left: (1.50px /* comment */ 2px);
		right: (1.50px) /* comment */;
		/* an escape is copied whole, so the number past its terminator is its own token */
		top: \31 .50px /* comment */ 2px;
	}
</style>
