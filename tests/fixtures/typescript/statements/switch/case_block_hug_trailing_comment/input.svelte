<script>
	function f(kind, fn) {
		switch (kind) {
			case 'a': {
				fn();
				break;
			} // block-hug trailing
			default:
				z();
		}
	}
</script>
