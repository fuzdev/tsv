<script>
	let cond = 1;

	// A block comment in the gap of a dropped `;`, on the line the previous statement
	// deferred its own line comment past.
	let a1 = 1 // c1
	; /* c2 */ ;
	let b1 = 2;

	// The same with a line comment after the dropped `;`.
	let a2 = 1 // c3
	; ; // c4
	let b2 = 2;

	// The same inside a block body.
	function fn() {
		let a3 = 1 // c5
		; /* c6 */ ;
		let b3 = 2;
	}

	// And inside a switch consequent.
	switch (cond) {
		case 1:
			let a4 = 1 // c7
			; /* c8 */ ;
			let b4 = 2;
	}
</script>
