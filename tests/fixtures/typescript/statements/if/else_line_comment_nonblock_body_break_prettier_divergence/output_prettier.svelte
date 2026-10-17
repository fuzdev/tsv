<script>
	// Line comment between else and a breaking non-block body - non-block consequent
	if (x) g();
	else
		// c
		items.forEach((item) => {
			use(item);
		});

	// Line comment between else and a breaking non-block body - block consequent
	if (x) {
		g();
	} else
		// c
		items.forEach((item) => {
			use(item);
		});
</script>
