<script lang="ts">
	// 100 chars - stays inline (at boundary)
	const fn = function <T extends string | number | boolean, U extends Record<string, u______>>() {};

	// 101 chars - wraps (exceeds print_width)
	const fn2 = function <
		T extends string | number | boolean,
		U extends Record<string, u_______>
	>() {};

	// Named function expression - 100 chars
	const named = function foo<T extends string | number | boolean, U extends Record<string, u>>() {};

	// Named function expression - 101 chars wraps
	const named2 = function foo<
		T extends string | number | boolean,
		U extends Record<string, u_>
	>() {};
</script>
