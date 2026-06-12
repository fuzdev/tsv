<style>
	/* @include/@mixin are SCSS directives, not standard CSS: per CSS Syntax 3 an unknown at-rule's prelude is an opaque token stream, so tsv keeps it verbatim — prettier recognizes SCSS and normalizes the numbers (see README). */
	@include foo(transform, .5s ease);
	@include baz(1.50);
	@mixin bar(.5s) {
		a: b;
	}
</style>
