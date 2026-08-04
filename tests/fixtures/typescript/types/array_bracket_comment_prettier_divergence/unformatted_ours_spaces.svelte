<script lang="ts">
	// A block comment inside an array type's brackets stays inside them.
	type A = string[   /* c */   ];

	// A line comment inside the brackets breaks them open, like an empty tuple type.
	type B = string[
			// c
			];

	// A comment before the brackets stays before them.
	type C = string   /* c */   [];

	// Each suffix in a chain of brackets keeps its own comment.
	type D = string[  /* c1 */  ][  /* c2 */  ];
</script>
