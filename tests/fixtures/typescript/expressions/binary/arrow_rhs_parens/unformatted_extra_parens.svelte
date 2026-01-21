<script lang="ts">
	// Arrow function in binary operators requires parens
	// (arrow has lower precedence than all binary operators)

	// Logical operators
	const a1 = b || (((fn) => fn));
	const a2 = b && (((fn) => fn));
	const a3 = b ?? (((fn) => fn));

	// Arithmetic operators
	const a4 = b + (((fn) => fn));
	const a5 = b - (((fn) => fn));
	const a6 = b * (((fn) => fn));
	const a7 = b ** (((fn) => fn));

	// Comparison operators
	const a8 = b > (((fn) => fn));
	const a9 = b === (((fn) => fn));
	const a10 = b instanceof (((fn) => fn));

	// Bitwise operators
	const a11 = b | (((fn) => fn));
	const a12 = b & (((fn) => fn));

	// Comma operator (arrow doesn't need extra parens - already grouped by outer parens)
	const a13 = (b, ((fn) => fn));

	// Arrow on LHS (parens needed to prevent || from being part of arrow body)
	const a14 = (((fn) => fn)) || b;

	// Chained logical with arrow at end
	const a15 = b || c || (((fn) => fn));

	// Multiple parameters
	const a16 = b || (((x, y) => x + y));

	// With return type
	const a17 = b || (((fn): T => fn));

	// Generic arrow (trailing comma disambiguates from JSX in Svelte context)
	const a18 = b || ((<T,>(fn: T): T => fn));

	// Async arrow
	const a19 = b || ((async (fn) => fn));

	// Block body (wraps to multiple lines)
	const a20 =
		b ||
		(((x) => {
			return x;
		}));

	// Destructuring parameter
	const a21 = b || ((({x}) => x));

	// Returning object literal
	const a22 = b || (((x) => ({y: x})));
</script>
