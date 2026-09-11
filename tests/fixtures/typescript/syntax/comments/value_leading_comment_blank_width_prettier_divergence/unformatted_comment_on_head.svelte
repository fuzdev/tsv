<script lang="ts">
	// variable initializer: a comment the author broke after, before a value that breaks by
	// width, takes its own line under `=`
	const u = /* c1 */
	aaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb + cccccccccccccccccccccccccc;

	// expression-statement assignment: same rule
	let w;
	w = /* c1 */
	aaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb + cccccccccccccccccccccccccc;

	// arrow body: same rule under `=>`
	const v = () => /* c1 */
	aaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb + cccccccccccccccccccccccccc;

	// arrow body that is a conditional: same rule
	const p = () => /* c1 */
	cccccccccccccccccccccccccccc ? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa : bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;

	// a glued run the author broke after: the run holds the line
	const s = /* c1 */ /* c2 */
	aaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb + cccccccccccccccccccccccccc;

	// a run whose last comment is glued to the value: the first comment takes its own line
	const q = /* c1 */
	/* c2 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb + cccccccccccccccccccccccccc;

	// the same run before a value that fits: one line
	const t = /* c1 */
	/* c2 */ b;

	// the same at a type alias
	type U = /* c1 */
	/* c2 */ B;

	// type alias union: same rule
	type A = /* c1 */
	Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | Cccccccccccccccccccccccccccccccccc | Dddddddddddddddddddddddd;
</script>
