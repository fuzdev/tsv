<script lang="ts">
	// Inline type modifier in named exports

	// Single inline type
	export { type A } from './a';

	// Mixed: type + value
	export { type B, c } from './b';

	// Inline type with alias
	export { type D as D2 } from './c';

	// Multiple inline types
	export { type E, type F } from './d';

	// Mixed with aliases
	export { type G as G2, h as h2 } from './e';
</script>
