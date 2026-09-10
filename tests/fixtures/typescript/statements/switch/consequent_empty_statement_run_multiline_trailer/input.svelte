<script lang="ts">
	// a multi-line block trailing a run of dropped `;`s runs to its own closing line, so a
	// comment the author glued past that `*/` trails the statement too — and the cursor
	// must not step back over either of them
	switch (a1) {
		case 1:
			fn1(); /* m1
			m2 */ // c1
			fn2();
		case 2:
			fn3();
	}

	// the last statement of a consequent
	switch (a2) {
		case 1:
			fn4(); /* m3
			m4 */ // c2
		case 2:
			fn5();
	}

	// one dropped `;`
	switch (a3) {
		case 1:
			fn6(); /* m5
			m6 */ // c3
			fn7();
		case 2:
			fn8();
	}

	// CONTROL: the same at program scope
	fn9(); /* m7
	m8 */ // c4
	fn10();

	// CONTROL: a block body
	function fn11() {
		fn12(); /* m9
		m10 */ // c5
		fn13();
	}
</script>
