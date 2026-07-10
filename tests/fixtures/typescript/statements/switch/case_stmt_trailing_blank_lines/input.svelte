<script>
	function f(kind, a, b) {
		switch (kind) {
			case 1:
				a(); // before blank

				b(); // after blank
				break;
		}
	}
</script>
