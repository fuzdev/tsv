<script lang="ts">
	// line comment between a default binding and the named-specifier brace
	import a, { b } from './a'; // c1

	// the same gap with a namespace binding
	import c, * as ns from './b'; // c2

	// a block keeps its side of the comma; the line comment trails past it
	import d /* c3 */, { e } from './c'; // c4

	// a run stays in order, each comment on its own line
	import f, { // c5
		// c6
		g
	} from './d';

	// the braces and their own breaks nest under the continuation
	import h, { // c7
		i, // c8
		j
	} from './e';

	// the whole header stays at ONE indent level, never a staircase
	import k, * as // c10 // c9
	ns2 from './f';
</script>
