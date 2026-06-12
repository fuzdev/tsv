<script lang="ts">
	//
	// Type-Only Exports
	//

	// Single type re-export
	export type {A} from './a';

	// With alias
	export type {B as B2} from './b';

	// Multiple types
	export type {C, D, E} from './c';

	// Multiple with aliases
	export type {F as F2, G as G2} from './d';

	// Star re-export as type
	export type * from './e';

	// Star re-export with alias
	export type * as Types from './f';

	// Default re-export as type
	export type {default as Default} from './g';

	// Mixed: default and named
	export type {default as H, I, J} from './h';
</script>
