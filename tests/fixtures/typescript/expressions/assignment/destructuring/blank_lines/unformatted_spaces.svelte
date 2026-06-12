<script lang="ts">
	// Object declaration with blank line between properties
	const {
		a,


		b,
	} = obj;

	// Assignment with blank line
	({
		x,


		y,
	} = obj);

	// With defaults and blank line
	const {
		first = 1,


		second = 2,
	} = obj;

	// Blank line before rest
	const {
		named,


		...rest
	} = obj;

	// Nested with blank lines preserved at each level
	const {
		outer: {
			inner,


			other,
		},
	} = obj;

	// Function params with blank line
	function fn({
		param1,


		param2,
	}: Props) {}

	// Multiple properties with strategic blank lines
	const {
		group1A,
		group1B,


		group2A,
		group2B,
	} = obj;

	// Blank line after comment
	const {
		foo, // comment


		bar,
	} = obj;

	// Blank line before comment
	const {
		one,


		// comment for two
		two,
	} = obj;

	// Arrow function params
	const arrow = ({
		a,


		b,
	}: Props) => {};

	// For-of destructuring
	for (const {
		x,


		y,
	} of items) {
	}

	// Catch clause
	try {
	} catch ({
		message,


		code,
	}) {}

	// With type annotation
	const {
		typed1,


		typed2,
	}: {typed1: string; typed2: number} = obj;

	// With satisfies
	const {
		sat1,


		sat2,
	} = obj satisfies SomeType;

	// Object in array - object preserves blank lines
	const [
		{
			nested1,


			nested2,
		},
	] = arr;

	// Array in object - array collapses (no blank line preservation in arrays)
	const {
		arr: [
			elem1,


			elem2,
		],
	} = obj;
</script>
