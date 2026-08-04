<script lang="ts">
	// A block comment inside the brackets of a parenthesized union element stays inside them.
	type A = ('a' | 'b')[/* c */];

	// A comment between the parens and the brackets stays between them.
	type B = ('a' | 'b') /* c */[];

	// A line comment inside the brackets breaks them open, like an empty tuple type.
	type C = ('a' | 'b')[
		// c
	];

	// A comment already inside the parens stays distinct from the suffix's own.
	type D = ('a' | 'b' /* i */)[/* c */];

	// The union's own parens expanding changes nothing about where the comment sits.
	type E = (
		'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' | 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'
	)[/* c */];
</script>
