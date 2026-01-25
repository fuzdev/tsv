<script>
	if (// before a
		a// inline with a
	) {
		fn1();
	} else if (// before b
		b// inline with b
	) {
		fn2();
	} else if (// before c
		c// inline with c
	) {
		fn3();
	} else {
		fn4();
	}

	// Block comment variant
	if (/* before */a/* after */) {
		g1();
	} else if (/* before */b/* after */) {
		g2();
	} else {
		g3();
	}
</script>
