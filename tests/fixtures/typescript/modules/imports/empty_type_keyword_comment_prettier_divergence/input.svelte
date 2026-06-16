<script lang="ts">
	// block comment between import and type keyword
	import /* c1 */ type {} from './a';

	// line comment between import and type keyword
	import // c2
		type {} from './b';

	// block comment between type keyword and braces
	import type /* c3 */ {} from './c';

	// line comment between type keyword and braces
	import type // c4
		{} from './d';
</script>
