<script>
	// A line comment on the init's `;` line, before an empty test slot, trails the init
	for (let i = 0 // forever
	; ; i++) {
		fn();
	}

	// Before an empty update slot, it trails the test
	for (a; b // c1
	; ) {
		fn();
	}

	// Both later slots empty
	for (a // c2
	; ; ) {
		fn();
	}

	// A non-block body
	for (let i = 0 // c3
	; ; i++) fn();
</script>
