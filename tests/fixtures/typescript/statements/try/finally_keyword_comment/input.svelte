<script>
	try {
	} finally /* finally */ {
	}

	try {
		a();
	} finally /* run finally here */ {
		b();
	}

	try {
		a();
	} catch (e) {
		c();
	} finally /* the finally clause */ {
		b();
	}
</script>
