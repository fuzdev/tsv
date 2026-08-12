<script lang="ts">
	// A comment glued past a multi-line block's closing `*/` trails the same gap the
	// block does — the anchor is that closing line, not the line the gap opened on.
	type T1 = {
		a: A /* x
		y */ /* c1 */;
		b: B;
	};

	interface I1 {
		a: A /* x
		y */ /* c2 */;
		b: B;
	}

	class C1 {
		a = 1 /* x
		y */ /* c3 */;
		b = 2;
	}

	// The same in a parameter list and in an array literal.
	function fn(
		aaaa /* x
		y */ /* c4 */,
		bbbb
	) {}

	const arr = [
		aaaa /* x
		y */ /* c5 */,
		bbbb
	];

	// And in a statement's own terminator gap.
	const expr = 1 /* x
	y */ /* c6 */;

	// Every other comma seam reads the same anchor: the object literal and its pattern,
	// the specifier list, the tuple, a type-parameter list, and an enum body.
	const obj = {
		aaaa: 1 /* x
		y */ /* c11 */
		,
		bbbb: 2
	};

	const {
		aaaa: cccc /* x
		y */ /* c12 */
		,
		bbbb: dddd
	} = obj;

	import {
		aaaa as eeee /* x
		y */ /* c13 */
		,
		bbbb as ffff
	} from './m';

	type Tup = [
		A /* x
		y */ /* c14 */
		,
		B
	];

	type TP<
		T /* x
		y */ /* c15 */
		,
		U
	> = [T, U];

	enum En {
		A /* x
		y */ /* c16 */
		,
		B
	}

	// A `//` glued past the run belongs to the same gap — the trailing arm claims it, not
	// the next item's leading run.
	function fnLine(
		aaaa /* x
		y */ /* c7 */ // c8
		,
		bbbb
	) {}

	const arrLine = [
		aaaa /* x
		y */ /* c9 */ // c10
		,
		bbbb
	];
</script>
