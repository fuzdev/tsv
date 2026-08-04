<script lang="ts">
	// At exactly 100 the parenthesized intersection stays inline with the comment after `)`
	type A = (Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa & Bbbbbbbbbbbbbbbbbbbbbbbbbbbb) /* c */[];
	// At 101 the intersection breaks inside its parens and the comment rides the closer line
	type B = (Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb) /* c */[];
	// At exactly 100 a `typeof` element stays inline
	type C = (typeof aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) /* c */[];
	// At 101 it has no internal break point, so the whole alias drops below the `=`
	type D =
		(typeof aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) /* c */[];
</script>
