<script lang="ts">
	// type alias value: the first comment's soft separator breaks with the value — the
	// second comment stays glued to it
	type A = /* c1 */
	/* c2 */ {
		a: 1;
		b: 2;
		c: 3;
	};

	// variable initializer: same rule
	const y = /* c1 */
	/* c2 */ {
		a: 1,
		b: 2
	};

	// expression-statement assignment: same rule as the declarator
	let w;
	w = /* c1 */
	/* c2 */ {
		a: 1,
		b: 2
	};

	// variable type annotation: the first comment keeps the head line, the glued one
	// opens the value on the next
	let x: /* c1 */
	/* c2 */ {
		a: 1;
		b: 2;
	};

	// function return type: same rule as the annotation
	function fn(): /* c1 */
	/* c2 */ {
		a: 1;
	} {
		return { a: 1 };
	}

	// arrow body: break after `=>`
	const v = () => /* c1 */
	/* c2 */ function () {
		b();
	};

	// a value that breaks by width alone: the soft separator breaks with the hang
	const u = /* c1 */
	/* c2 */ aaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb + cccccccccccccccccccccccccc;

	// a glued run the author broke after holds its own line above a width-broken value
	const s = /* c1 */ /* c2 */
	aaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb + cccccccccccccccccccccccccc;

	// same at the arrow body
	const r = () => /* c1 */ /* c2 */
	function () {
		b();
	};

	// the break between the comments carries an author blank when the value breaks
	let z: /* c1 */

	/* c2 */ {
		a: 1;
	};

	// a type the width breaks: the glued comment stays on the value's opening line
	type V = /* c1 */
	/* c2 */ [Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, Cccccccccccccccccccccccccccccc];

	// redundant grouping parens around the value: same layout as the bare value
	const k = /* c1 */
	/* c2 */ ({
		a: 1,
		b: 2
	});
	const m = /* c1 */
	/* c2 */ (aaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb + cccccccccccccccccccccccccc);

	// an arrow's object body keeps its parens; the glued comment stays ahead of them
	const g = () => /* c1 */
	/* c2 */ ({
		b: 1,
		c: 2
	});

	// a value that fits collapses the soft separator to a space
	const t = /* c1 */
	/* c2 */ b;
</script>
