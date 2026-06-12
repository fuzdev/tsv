<script lang="ts">
	// Array destructuring - basic
	[a, b] = arr;
	[a, ...rest1] = arr;
	[a = 1, b = 2] = arr;
	[[nested]] = arr;

	// Array holes (elision)
	[, a] = arr;
	[a, ] = arr;
	[first, , third] = arr;
	[, , third] = arr;
	[a, , , d] = arr;
	[, ...rest2] = arr;
	[, , ...rest3] = arr;
	[a, , ...rest4] = arr;
	[, , a, , b] = arr;

	// Object destructuring
	({a, b} = obj);
	({a: x, b: y} = obj);
	({a = 1, b = 2} = obj);
	({a, ...rest5} = obj);
	({outer: {inner}} = obj);

	// Empty patterns
	({} = obj);
	[] = arr;

	// Rest-only
	({...rest6} = obj);
	[...rest7] = arr;

	// Computed keys
	({[key]: x} = obj);
	({[0]: first, [1]: second} = obj);

	// Numeric/string keys
	({0: first, 1: second} = obj);
	({"key": value} = obj);

	// Deep nesting
	[[a1, b1], [c1, d1]] = arr;
	({a: {b: {c: x}}} = obj);

	// Nested with defaults
	[[a = 1]] = arr;
	({a: {b = 1}} = obj);

	// Trailing commas
	[a, b,] = arr;
	({a, b,} = obj);

	// Mixed nesting
	({arr: [first, second]} = obj);
	[{a}, {b}] = arr;
</script>
