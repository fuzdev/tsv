<script>
	// a dropped `;` is not a statement: a block it leaves alone in the case keeps the label
	// line, whichever side of the block the `;` was written on
	switch (x) {
		case 'a':
			;
			{
				fn1();
			}
		case 'b': {
			fn2();
		};
	}
</script>
