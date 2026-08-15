<script lang="ts">
	// line comment between a default binding and the named-specifier brace
	import a // c1
		, { b } from './a';

	// the same gap with a namespace binding
	import c // c2
		, * as ns from './b';

	// a block keeps its side of the comma; the line comment trails past it
	import d /* c3 */ // c4
		, { e } from './c';

	// a run stays in order, each comment on its own line
	import f, // c5
		// c6
		{ g } from './d';

	// the braces and their own breaks nest under the continuation
	import h, // c7
		{
			i, // c8
			j
		} from './e';

	// the whole header stays at ONE indent level, never a staircase
	import k, // c9
		* as // c10
		ns2 from './f';
</script>
