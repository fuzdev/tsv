<script lang="ts">
	// block comment between import and type keyword
	import type {/* c1 */ A} from './a';

	// block comment between type keyword and braces
	import type {/* c2 */ B} from './b';

	// line comment between import and type keyword
	import type {
		// c3
		C,
	} from './c';

	// line comment between type keyword and braces
	import type {
		// c4
		D,
	} from './d';
</script>
