<script lang="ts">
	// A generic check or extends type — a reference with type arguments, or a function type with
	// type parameters — makes the alias break after `=` instead of at `?` / `:`. A redundant paren
	// shell around either (stripped on output) must not change that verdict.
	type Equals<A, B> =
		(<T>() => T extends B ? 1 : 0) extends <T>() => T extends A ? 1 : 0 ? true : false;
	type Weird<A> =
		(<U extends boolean>(a: U) => never) extends <U extends true>(a: U) => never ? never : never;

	// a parenthesized generic reference on either side
	type Rxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx<A> =
		Foo<A> extends Bar ? Trueeeeeeeee : Falseeeeeeee;
	type Sxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx<A> =
		Foo extends Bar<A> ? Trueeeeeeeee : Falseeeeeeee;

	// fits: stays on one line, parens stripped
	type Short<A, B> = (<T>() => A) extends <T>() => B ? true : false;
</script>
