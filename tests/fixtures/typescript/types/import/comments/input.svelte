<script lang="ts">
	// leading line comment - breaks to multiline
	type A = import(
		// c
		'a'
	);

	// leading block comment on its own line - breaks
	type B = import(
		/* c */
		'a'
	);

	// leading block comment inline - stays inline
	type C = import(/* c */ 'a');

	// trailing line comment - breaks to multiline
	type D = import(
		'a' // c
	);

	// trailing block comment inline - stays inline
	type E = import('a' /* c */);

	// leading comment with qualifier - stays inline
	type F = import(/* c */ 'a').Foo;

	// leading comment with qualifier and type arguments
	type G = import(/* c */ 'a').Foo<string>;
</script>
