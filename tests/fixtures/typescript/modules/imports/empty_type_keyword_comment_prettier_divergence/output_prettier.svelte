<script lang="ts">
	// block comment between import and type keyword
	import type {} from /* c1 */ './a';

	// line comment between import and type keyword
	import type {} from // c2
	'./b';

	// block comment between type keyword and braces
	import type {} from /* c3 */ './c';

	// line comment between type keyword and braces
	import type {} from // c4
	'./d';
</script>
