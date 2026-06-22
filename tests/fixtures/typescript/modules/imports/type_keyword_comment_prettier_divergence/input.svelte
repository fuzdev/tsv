<script lang="ts">
	// block comment between import and type keyword
	import /* c1 */ type { A } from './a';

	// block comment between type keyword and braces
	import type /* c2 */ { B } from './b';

	// line comment between import and type keyword
	import // c3
		type { C } from './c';

	// line comment between type keyword and braces
	import type // c4
		{ D } from './d';
</script>
