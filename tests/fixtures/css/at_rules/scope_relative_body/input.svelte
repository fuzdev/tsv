<style>
	/* @scope body is a relative-selector-list scoped to :scope (CSS Cascade 6
	   §"Scoped Style Rules"): a bare declaration applies to :scope, a plain
	   selector is implicitly :scope-relative, and a leading combinator is allowed. */
	@scope (.a) {
		color: green;
		span {
			color: red;
		}
		> p {
			color: blue;
		}
		> div:nth-of-type(2) > span {
			z-index: 1;
		}
	}
	@scope {
		> div {
			color: red;
		}
	}
</style>
