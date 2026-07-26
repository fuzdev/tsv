<script lang="ts">
	// an own-line directive after the `{` freezes only the first specifier
	import {
		// prettier-ignore
		aaa   as   a1,
		bbb as b1
	} from './a';

	// an own-line block comment behaves identically
	import {
		/* prettier-ignore */
		ccc   as   c1,
		ddd as d1
	} from './b';

	// a default binding beside the named list
	import e1, {
		// prettier-ignore
		fff   as   f1,
		ggg as g1
	} from './c';

	// an inline `type` modifier rides inside the frozen slice
	import {
		// prettier-ignore
		type   H,
		iii as i1
	} from './d';

	// a lone specifier
	import {
		// prettier-ignore
		jjj   as   j1
	} from './e';

	// a string specifier
	import {
		// prettier-ignore
		'k-k'   as   k1
	} from './f';

	// a type-only import
	import type {
		// prettier-ignore
		LLL   as   L1,
		MMM as M1
	} from './g';
</script>
