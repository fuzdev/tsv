<script lang="ts">
	// block comment between export and type keyword
	export /* c1 */ type {} from './a';

	// line comment between export and type keyword
	export // c2
	type {} from './b';

	// block comment between type keyword and braces
	export type /* c3 */ {} from './c';

	// line comment between type keyword and braces
	export type // c4
	{} from './d';
</script>
