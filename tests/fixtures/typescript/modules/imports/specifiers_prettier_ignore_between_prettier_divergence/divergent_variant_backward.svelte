<script lang="ts">
	// an own-line directive between two specifiers freezes the following specifier;
	// the preceding one reformats
	import {
		aaa   as   a1,
		// prettier-ignore
		bbb as b1
	} from './a';

	export {
		ccc   as   c1,
		// prettier-ignore
		ddd as d1
	} from './b';
</script>
