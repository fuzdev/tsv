<script lang="ts">
	// Block comment after ?
	type A = B extends T ? /* c1 */ foo : bar;

	// Block comment after :
	type B = B extends T ? foo : /* c2 */ bar;

	// Block comments after both ? and :
	type C = B extends T ? /* c1 */ foo : /* c2 */ bar;

	// Line comment after ? (forces multiline)
	type D = B extends T
		? // line comment
			foo
		: bar;

	// Line comment after : (forces multiline)
	type E = B extends T
		? foo
		: // line comment
			bar;

	// Line comments after both ? and : (forces multiline)
	type F = B extends T
		? // c1
			foo
		: // c2
			bar;

	// Nested conditional with comments
	type G = A extends B ? /* c1 */ foo : C extends D ? /* c2 */ bar : /* c3 */ baz;

	// Comment between extends keyword and type (in condition)
	type H = B extends /* c */ T ? foo : bar;
</script>
