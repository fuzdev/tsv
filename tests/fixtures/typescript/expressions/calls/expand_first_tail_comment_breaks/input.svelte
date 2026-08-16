<script>
	// a multiline block comment leading the tail argument breaks the tail, so every
	// argument breaks out. The run's head is UNOWNED (a second block sits between it
	// and the argument), so it rides the gap rather than the argument's own doc — the
	// break question has to ask the whole tail, not just the argument
	fn(
		() => {
			a();
		},
		/* c1
c2 */ /* c3 */ b
	);

	new A(
		() => {
			a();
		},
		/* c1
c2 */ /* c3 */ b
	);

	obj.fn1(1).fn2(
		() => {
			a();
		},
		/* c1
c2 */ /* c3 */ b
	);

	// control: a single block comment leading the tail is OWNED by the argument and
	// rides inside its doc, so the argument's own break question already sees it
	a.b(
		() => {
			a();
		},
		/* c1
c2 */ b
	);

	// control: a run that stays flat keeps the expand-first hug
	obj.fn1(1).fn2(() => {
		a();
	}, /* c1 */ /* c2 */ b);
</script>
