<script>
	// A block comment glued to the body
	do /* c */ items.forEach((item) => {
		use(item);
	});
	while (more());

	// A trailing comment after the body
	do items.forEach((item) => {
		use(item);
	}); // tail
	while (more());

	// A block comment on a flat body stays on the keyword's line
	do /* c */ g();
	while (more());

	// A line comment after do
	do // c
		items.forEach((item) => {
			use(item);
		});
	while (more());
</script>
