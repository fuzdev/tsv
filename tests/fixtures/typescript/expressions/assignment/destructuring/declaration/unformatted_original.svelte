<script lang="ts">
	// Array declarations - basic
	const [a1, b1] = arr;
	const [x1, ...rest1] = arr;
	let [c1 = 1, d1 = 2] = arr;
	const [[nested1]] = arr;

	// Array holes (elision)
	const [, a2] = arr;
	const [b2, ] = arr;
	let [first1, , third1] = arr;
	const [, , third2] = arr;
	let [e1, , , f1] = arr;
	const [, ...rest2] = arr;
	const [, , ...rest3] = arr;
	let [g1, , ...rest4] = arr;
	const [, , h1, , i1] = arr;

	// Object declarations
	const {a: a3, b: b3} = obj;
	let {a: x2, b: y2} = obj;
	const {c: c2 = 1, ...rest5} = obj;

	// Empty patterns
	const {} = obj;
	const [] = arr;

	// Rest-only
	const {...rest6} = obj;
	const [...rest7] = arr;

	// Computed keys
	const {[key]: j1} = obj;

	// Numeric/string keys
	const {0: first3} = obj;
	const {"key": value1} = obj;

	// Deep nesting
	const [[k1, l1], [m1, n1]] = arr;
	const {a: {b: {c: o1}}} = obj;

	// Nested with defaults
	const [[p1 = 1]] = arr;
	const {a: {b: q1 = 1}} = obj;

	// Nested
	const {outer: {inner: inner1}} = obj;
	const [[nested2]] = arr;

	// Mixed nesting
	const {arr: [first2, second2]} = obj;
	let [{a: a4}, {b: b4}] = arr;
</script>
