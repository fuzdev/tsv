<script>
	// An own-line block comment with the comma glued after leads the next specifier
	import { a, /* c */ b } from './a';

	// Written before the comma instead, it trails the previous specifier
	import { c1 /* c */, d } from './b';

	// A line comment expands the list - the comment still leads its specifier, on its line
	import {
		e,
		/* c */ f // t
	} from './c';

	fn(a, b, c1, d, e, f);
</script>
