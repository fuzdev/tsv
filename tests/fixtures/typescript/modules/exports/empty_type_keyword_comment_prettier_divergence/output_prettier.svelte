<script lang="ts">
	// block comment between export and type keyword
	export type {} from /* c1 */ './a';

	// line comment between export and type keyword
	export type {} from // c2
	'./b';

	// block comment between type keyword and braces
	export type {} from /* c3 */ './c';

	// line comment between type keyword and braces
	export type {} from // c4
	'./d';
</script>
