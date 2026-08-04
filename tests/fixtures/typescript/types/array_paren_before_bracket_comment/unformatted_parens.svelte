<script lang="ts">
	// A comment between an element's parens and its brackets stays between them.
	type A = ((X & Y)) /* c */[];

	// The parens tsv adds around a bare `typeof` element land before the comment.
	type B = typeof x /* c */[];

	// A hugged union element behaves the same.
	type C = (T | null)  /* c */  [];
</script>
