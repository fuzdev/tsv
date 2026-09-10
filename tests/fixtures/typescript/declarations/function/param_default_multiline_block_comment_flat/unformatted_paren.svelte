<script lang="ts">
	// A multi-line block comment leading a binding default's value must not BREAK the
	// value's own group: prettier prints a leading comment outside that group, so a
	// value that fits stays flat below the comment. The default keeps `= value` inline
	// (it never hangs under the `=`), so this seam reads the comment for itself.

	// binary chain
	function fn1(
		a1 = /**
		 * c
		 */ (b1 + c1 + d1)
	) {}

	// ternary
	function fn2(
		a2 = /**
		 * c
		 */ (b2 ? c2 : d2)
	) {}

	// member chain
	function fn3(
		a3 = /**
		 * c
		 */ b3.c3().d3().e3()
	) {}

	// a preserved (non-indentable) multi-line block asks the same question
	function fn4(
		a4 = /* c
	d */ (b4 + c4 + d4)
	) {}

	// object destructuring binding default
	const {
		x1 = /**
		 * c
		 */ (y1 + z1 + w1)
	} = o;

	// array destructuring binding default
	const [
		x2 = /* c
	d */ (y2 + z2 + w2)
	] = arr;

	// null control: a single-line block forces no break, so the list stays flat
	function fn5(a5 = /* c */ b5 + c5 + d5) {}

	// null control: no comment
	function fn6(a6 = b6 + c6 + d6) {}
</script>
