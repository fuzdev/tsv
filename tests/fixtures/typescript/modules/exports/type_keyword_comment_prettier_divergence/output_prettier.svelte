<script lang="ts">
	// block comment between export and type keyword
	export type { /* c1 */ A } from './a';

	// block comment between type keyword and braces
	export type { /* c2 */ B } from './b';

	// line comment between export and type keyword
	export type {
		// c3
		C
	} from './c';

	// line comment between type keyword and braces
	export type {
		// c4
		D
	} from './d';
</script>
