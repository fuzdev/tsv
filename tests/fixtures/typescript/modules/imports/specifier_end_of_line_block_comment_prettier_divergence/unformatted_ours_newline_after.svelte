<script>
	// A block comment after the comma leads the next specifier - it stays there
	import { a1, /* c */
b1 } from './a';

	// Exports take the same rule
	export { a2, /* c */
b2 };

	// Contrast - a comment before the comma trails the previous specifier
	import { a3 /* c */, b3 } from './b';

	// A line comment expands the list - the glued comment still leads its specifier
	import {
		a4,
		/* c1 */ b4, // c2
		c4
	} from './c';
</script>
