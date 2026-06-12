<script lang="ts">
	// comment between import and default binding
	import /* c1 */ Foo from './a';

	// comment between import and type keyword
	import /* c2 */ type Bar from './b';

	// comment between type keyword and default binding
	import type /* c3 */ Baz from './c';

	// line comment between import and type keyword
	import // c4
	type Qux from './d';
</script>
