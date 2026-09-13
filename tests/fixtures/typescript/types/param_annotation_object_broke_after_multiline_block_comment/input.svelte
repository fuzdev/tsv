<script lang="ts">
	// A multi-line block the author broke after, ahead of an object-typed parameter
	// annotation: the object does not hug the comment's closing line — the break after
	// the `*/` is kept, as at every other annotation.
	function a(b: /* c1
	d1 */
	{
		e: 1;
	}) {}

	// a variable annotation, the same
	let f: /* c2
	d2 */
	{ g: 1 };

	// the indentable kind
	function h(i: /**
	 * c3
	 */
	{
		j: 1;
	}) {}

	// a run: a single-line block glued ahead of the multi-line one
	function k(l: /* c4 */ /* c5
	d5 */
	{
		m: 1;
	}) {}

	// control: a multi-line block glued to the object hugs
	function n(o: /* c6
	d6 */ {
		p: 1;
	}) {}

	// control: a single-line block the author broke after collapses
	function q(r: /* c7 */ { s: 1 }) {}
</script>
