<script>
	function f(kind, a, b) {
		switch (kind) {
			case 1:
				a(); // trailing
				// own-line follows
				b();
				break;
		}
	}
</script>
