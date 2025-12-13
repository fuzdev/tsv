<script lang="ts">
	// Basic import type
	type A = import('a');

	// With qualifier
	type B = import('./b').T;

	// With type arguments
	type C = import('c').Type<string>;

	// With multiple type arguments
	type D = import('d').Type<T, U, V>;

	// Qualified name (nested)
	type E = import('./e').A.B.C;

	// In function parameter and return type
	function fn(x: import('./f').T): import('./f').U {
		return x;
	}

	// In generic constraint
	type F<T extends import('./g').Base> = {value: T};

	// Array of import type
	type G = import('h').T[];

	// Union with import types
	type H = import('i').T | import('j').U;

	// Intersection with import types
	type I = import('k').T & import('l').U;
</script>
