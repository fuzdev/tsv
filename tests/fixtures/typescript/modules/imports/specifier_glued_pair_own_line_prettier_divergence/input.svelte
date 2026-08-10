<script>
	// A glued pair of block comments after the comma leads the next specifier and stays
	// there, on the specifier's line
	import { a1, /* c1 */ /* c2 */ b1 } from './a';

	// Exports take the same rule
	export { a2, /* c1 */ /* c2 */ b2 };
</script>
