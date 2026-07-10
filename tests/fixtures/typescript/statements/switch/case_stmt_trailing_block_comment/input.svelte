<script>
	function f(kind, a, b, c) {
		switch (kind) {
			case 1:
				a(); /* interior block */
				b(); /* set */ // then line
				c();
				break;
			case 2:
				a(); /* multi
				line block */
				b();
				break;
			case 3:
				return c; /* last-of-case block */
			default:
				z(); /* last-of-switch block */
		}
	}
</script>
