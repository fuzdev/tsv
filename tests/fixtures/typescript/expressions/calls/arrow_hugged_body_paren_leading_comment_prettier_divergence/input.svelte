<script>
	// A block comment the author wrote INSIDE an arrow's object-body parens stays inside
	// them, in every printer that hugs such a body: sole argument, `new`, member chain.

	const b1 = fn((item) => (/* c */ {
		prop1: 'value1value1value1value1',
		prop2: 'value2value2value2value2',
		prop3: 'value3'
	}));
	const b2 = new Fn((item) => (/* c */ {
		prop1: 'value1value1value1value1',
		prop2: 'value2value2value2value2',
		prop3: 'val'
	}));
	const b3 = obj.fn1().map((item) => (/* c */ {
		prop1: 'value1value1value1value1',
		prop2: 'value2value2value2value2',
		prop3: 'v3'
	}));

	// Control: written outside the parens, it stays outside — the two positions are
	// dual-stable, and each formatter keeps the one the author picked.
	const b4 = obj.fn1().map((item) => /* c */ ({
		prop1: 'value1value1value1value1',
		prop2: 'value2value2value2value2',
		prop3: 'v3'
	}));
</script>
