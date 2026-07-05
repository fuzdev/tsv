<script>
	// own-line block comment in the init→`;` gap moves to its own line after `;`
	for (let i = 0
		/* c1 */
		; i < 10; i++) {
		fn();
	}

	// own-line line comment in the init→`;` gap moves to its own line after `;`
	for (let j = 0
		// c2
		; j < 10; j++) {
		fn();
	}

	// a same-line block stays before `;`; a following own-line block defers after `;`
	for (let k = 0 /* c3 */
		/* c4 */
		; k < 10; k++) {
		fn();
	}

	// own-line block comment in the test→`;` gap moves to its own line after `;`
	for (let m = 0; m < 10
		/* c5 */
		; m++) {
		fn();
	}
</script>
