<script>
	function fn() {
		switch (x) {
// before first case
			case 1:
// before first stmt
				a();
// before second stmt
				b();
				break;
// between case 1 and case 2
			case 2:// inline after case label
// before throw
				throw new Error('x');
// between case 2 and default
			default:
// before return
				return;
// trailing after last case
		}
	}

	function fn2() {
		switch (y) {
			case 3:// fallthrough
			case 4:
				c();
				break;
		}
	}
</script>
