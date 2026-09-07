<style>
	div {
		/* a value holding a comment still takes every value-level rule the comment-free
		path takes: a number canonicalizes, a hex colour lowercases, a string re-quotes
		and a unit folds to its canonical case */
		margin: 1.5px /* comment */ 2px;
		padding: 0.5px /* comment */ 2px;
		border-color: #FFF /* comment */;
		font-family: 'a' /* comment */;
		width: 1PX /* comment */ 2px;
		line-height: 1e3 /* comment */;
		/* the rules reach a function's own argument, and a trailing comment does not
		stop the value before it */
		height: calc(1.5px + 2.5px) /* comment */;
		background-color: rgba(0, 0, 0, 0.5 /* comment */);
		/* a custom property takes the same path */
		--a: 1.5px /* comment */ 2px;
		/* an unquoted url() is opaque, so its content is copied whether or not the
		value carries a comment */
		background-image: url(a1.50.png) /* comment */;
		/* an escape is copied whole, so the number past its terminator is its own token */
		top: \31 0.5px /* comment */ 2px;
	}
</style>
