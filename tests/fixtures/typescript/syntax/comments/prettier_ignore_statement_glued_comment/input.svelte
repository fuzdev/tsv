<script>
	// a block comment glued before a frozen statement stays glued to it — the slice
	// begins at the statement, so the comment prints just ahead of it
	fn(aaa);
	// prettier-ignore
	/* c */ fn(  bbb  );

	// same rule inside a block body
	function fn1() {
		fn(aaa);
		// prettier-ignore
		/* c */ fn(  bbb  );
	}

	// a switch case's consequent list
	switch (aaa) {
		case 1:
			// prettier-ignore
			/* c */ fn(  bbb  );
	}

	// a labeled statement's body
	lll:
	// prettier-ignore
	/* c */ fn(  bbb  );

	// a labeled body that opens with a keyword keeps the comment on its line too
	mmm:
	// prettier-ignore
	/* c */ for (;;) {
		fn(  bbb  );
		break mmm;
	}
</script>
