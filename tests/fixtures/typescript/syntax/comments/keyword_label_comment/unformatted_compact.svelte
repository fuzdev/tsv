<script>
	// Block comment between keyword and label
	loop1: for (;;) {
		break/* c */loop1;
	}

	loop2: for (;;) {
		continue/* c */loop2;
	}

	// Block comment between label and semicolon
	loop3: for (;;) {
		break loop3/* c */;
	}

	loop4: for (;;) {
		continue loop4/* c */;
	}

	// Line comment after label (forces line break)
	loop5: for (;;) {
		break loop5;// c
	}

	loop6: for (;;) {
		continue loop6;// c
	}
</script>
