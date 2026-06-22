<script lang="ts">
	import { a, b } from './mod';
	import type { T } from './types';
	import * as ns from './ns';
	import def from './def';

	export { a, b };
	export type { T };
	export const x = 1;
</script>

<div>{a}</div>
