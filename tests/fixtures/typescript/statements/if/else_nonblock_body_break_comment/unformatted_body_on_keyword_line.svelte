<script>
	// A comment before else
	if (x) g();
	// c
	else items.forEach((item) => {
		use(item);
	});

	// A block comment glued to the body
	if (x) g();
	else /* c */ items.forEach((item) => {
		use(item);
	});

	// A trailing comment after the body
	if (x) g();
	else items.forEach((item) => {
		use(item);
	}); // tail

	// A block comment on a flat body stays on the keyword's line
	if (x) g();
	else /* c */ g();
</script>
