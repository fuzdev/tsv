<script>
	function fn() {
		a();
		// trailing comment
	}

	const fn2 = () => {
		b();
		// trailing in arrow
	};

	if (cond) {
		c();
		// trailing in if
	}

	for (const x of items) {
		d();
		// trailing in for
	}

	while (cond) {
		e();
		// trailing in while
	}

	do {
		f();
		// trailing in do-while
	} while (cond);

	try {
		g();
		// trailing in try
	} catch (e) {
		h();
		// trailing in catch
	} finally {
		i();
		// trailing in finally
	}

	// Block comment variant
	function fn3() {
		j();
		/* trailing block */
	}

	if (cond) {
		k();
		/* trailing block in if */
	}
</script>
