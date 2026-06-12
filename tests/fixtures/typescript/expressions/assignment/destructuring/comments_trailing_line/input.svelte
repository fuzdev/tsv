<script lang="ts">
	// trailing line comment after property with more properties following
	const {
		a,
		b = 'value', // trailing line comment
		c = true,
		...rest
	} = $props();

	// trailing after last property
	const {d, e} = x; // trailing on whole statement

	// line comment after specific property
	const {
		f, // comment after f
		g, // comment after g
		h,
	} = y;

	// mixed block and line
	const {
		i = 1 /* block */, // line
		j,
	} = z;

	// array destructuring with trailing line comments
	const [
		aa, // comment after aa
		bb, // comment after bb
		...arrRest // comment after rest
	] = arr;

	// nested destructuring
	const {
		outer: {
			inner, // comment on inner
		}, // comment on outer
	} = nested;

	// renamed with line comment
	const {
		oldName: newName, // renaming comment
		xx,
	} = obj;

	// line comment on last property before closing brace
	const {
		first,
		last, // this is the last one
	} = obj2;

	// function params with destructuring
	function fn({
		param1, // first param
		param2, // second param
	}: Props) {}
</script>
