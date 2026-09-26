<script lang="ts">
	// A comment inside the kept parens stays inside them
	type A = (/* c */ -1)[];
	type B = (-1 /* c */)[K];
	type C = (-/* c */ 1)[];
	type D = (-// c
	1)[];
	type E = ( // c
		-1
	)[];
	type F = (
		-1 // c
	)[K];

	// A comment after the kept parens stays after them
	type G = (-1) /* c */[];
	type H = (-1)[/* c */];
</script>
