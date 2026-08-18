<script>
	// last case: the `;`-line comment lands own-line at the case's level
	switch (a) {
		case 1:
			fn1() // c1
			; // c2
	}
	// the same landing past a dropped `;` (an empty statement carried the comment)
	switch (b) {
		case 1:
			fn2() // c3
			;; // c4
	}
	// a sibling case bounds the same landing
	switch (c) {
		case 1:
			fn3() // c5
			; // c6
		case 2:
			fn4();
	}
	// a block trailing the `;` lands own-line at the case's level too
	switch (d) {
		case 1:
			fn5() // c7
			; /* c8 */
	}
</script>
