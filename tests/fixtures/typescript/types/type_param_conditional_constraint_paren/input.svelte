<script lang="ts">
	// Conditional type in `extends` constraint: parens kept
	type X<Foo extends (Bar extends Baz ? A : B)> = Foo;
	// Conditional type in `=` default: parens stripped
	type Y<Foo = Bar extends Baz ? A : B> = Foo;
	// Nested conditional in constraint: parens kept
	type Z<Foo extends (Bar extends Baz ? (C extends D ? E : F) : G)> = Foo;
</script>
