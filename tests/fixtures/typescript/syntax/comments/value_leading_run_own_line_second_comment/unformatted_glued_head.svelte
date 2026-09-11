<script lang="ts">
	// assignment: a comment on its own line breaks the separator of the comment before it
	let a;
	a = /* c1 */
	/* c2 */
	b;

	// the forced break carries an author blank after the first comment
	let w;
	w = /* c1 */

	/* c2 */
	b;

	// a glued pair on its own line before a value the width breaks
	let s;
	s = /* c1 */
	/* c2 */ /* c3 */
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb + cccccccccccccccccccccccccccc;

	// spread argument: same rule
	const x = [.../* c1 */
	/* c2 */
	b];

	// an arrow call argument whose body is a conditional
	new C((y) => /* c1 */
	/* c2 */
	aaa ? bbb : ccc);

	// a line comment on its own line breaks the separator of the block before it too
	const y = [.../* c1 */
	// c2
	b];

	// the forced break carries an author blank there as well
	const z = [.../* c1 */

	// c2
	b];

	new D((q) => /* c1 */
	// c2
	aaa ? bbb : ccc);

	// type alias value: the first comment moves below `=` with the rest of the run
	type T = /* c1 */
	/* c2 */
	B;

	// variable initializer: same rule
	const v = /* c1 */
	/* c2 */
	b;
</script>
