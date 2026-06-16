<script lang="ts">
	// comment between import and namespace star
	import /* c1 */ * as ns1 from './a';

	// comment between import and type keyword
	import /* c2 */ type * as ns2 from './b';

	// comment between type keyword and namespace star
	import type /* c3 */ * as ns3 from './c';

	// line comment between import and type keyword
	import // c4
		type * as ns4 from './d';

	// comment between namespace star and as
	import * /* c5 */ as ns5 from './e';

	// line comment between import and namespace star
	import // c6
		* as ns6 from './f';

	// line comment between type keyword and namespace star
	import type // c7
		* as ns7 from './g';
</script>
