<script>
	// the expand-last hug survives a multiline string in the last argument
	fn1(a, {key: 'multi \
line'});

	// an array last argument hugs the same way
	fn1(a, ['multi \
line']);

	// the `new` twin takes the same layout
	const b = new Fn1(a, {key: 'multi \
line'});

	// the same call inside a member chain
	obj.fn1(a, {key: 'multi \
line'});

	// a template literal's own newline is the same trigger
	fn1(a, {key: `tab\there
line`});

	// control - a multiline string in a HEAD argument still breaks every argument out
	fn1({key: 'multi \
line'}, b);
</script>
