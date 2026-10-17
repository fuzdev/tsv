<script>
	// A callback body that breaks drops the else body below the keyword
	if (x) g();
	else items.forEach((item) => {
		use(item);
	});

	// An object argument the printer keeps expanded
	if (x) g();
	else configure({
		retries: 3
	});

	// An assignment whose object value stays expanded
	if (x) g();
	else options = {
		retries: 3
	};

	// A template literal spanning lines
	if (x) g();
	else log(`first
second`);

	function f() {
		// A return argument that breaks
		if (x) g();
		else return (
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa ||
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		);

		// A throw argument that breaks
		if (x) g();
		else throw new Error(
			'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' +
				bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		);
	}

	// A block consequent
	if (x) {
		g();
	} else items.forEach((item) => {
		use(item);
	});

	// The final else of an else-if chain
	if (x) g();
	else if (y) h();
	else items.forEach((item) => {
		use(item);
	});

	async function f2() {
		// An awaited call whose argument breaks
		if (x) g();
		else await save({
			retries: 3
		});
	}

	// A body that fits flat stays on the keyword's line
	if (x) g();
	else configure({ retries: 3 });

	if (x) g();
	else items.forEach((item) => use(item));

	for (;;) {
		if (x) g();
		else break;
	}

	// An else-if keeps its own clause
	if (x) g();
	else if (y)
		items.forEach((item) => {
			use(item);
		});
</script>
