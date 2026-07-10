<script>
	function classify(kind, a, l, parent) {
		switch (kind) {
			case R_GENERIC_CALL:
				l.close(a); // close T_GENERIC
				l.close(a); // close T_GENERIC_FUNCTION
				parent.i = a; // set index
				break; // fallout
			case R_RETURN:
				return c === 62 ? 2 : 1; // =>
			default:
				return 1; // ~
		}
	}
</script>
