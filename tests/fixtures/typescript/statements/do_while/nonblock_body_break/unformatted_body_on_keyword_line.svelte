<script>
	// A callback body that breaks drops the do body below the keyword
	do items.forEach((item) => {
		use(item);
	});
	while (more());

	// An object argument the printer keeps expanded
	do configure({
		retries: 3
	});
	while (more());

	// A template literal spanning lines
	do log(`first
second`);
	while (more());

	// Arguments that break
	do send(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
	);
	while (more());

	// A body that fits flat stays on the keyword's line
	do configure({ retries: 3 });
	while (more());

	do i++;
	while (i < 10);
</script>
