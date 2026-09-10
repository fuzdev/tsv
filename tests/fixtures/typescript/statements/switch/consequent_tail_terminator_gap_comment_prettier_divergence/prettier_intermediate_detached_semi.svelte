<script lang="ts">
	// an own-line `//` in the last statement's terminator gap
	switch (a1) {
		case 1:
			fn1();
			// c1
		case 2:
			fn2();
	}

	// the LAST case takes the same landing
	switch (a2) {
		case 1:
			fn3();
			// c2
	}

	// a run of them, the last one a block
	switch (a3) {
		case 1:
			fn4();
			// c3
			/* c4 */
		case 2:
			fn5();
	}

	// past a dropped `;`
	switch (a4) {
		case 1:
			fn6();
			// c5
		case 2:
			fn7();
	}

	// CONTROL: with a following statement in the same consequent the gap comment leads
	// it, at the consequent's own level
	switch (a5) {
		case 1:
			fn8();
			// c6
			fn9();
		case 2:
			fn10();
	}

	// CONTROL: a block body's last statement already lands its gap comment in place
	function fn11() {
		fn12();
		// c7
	}
</script>
