<script>
	// a same-line line comment before the comma trails the specifier
	import { a1 // c1
, b1 } from './a';

	// an own-line line comment before the comma leads the next specifier
	import { a2
// c1
, b2 } from './b';

	// an own-line block comment before the comma leads the next specifier too
	import { a3
/* c1 */
, b3 } from './c';

	// exports take the same partition
	export { a4 // c1
, b4 };
</script>
