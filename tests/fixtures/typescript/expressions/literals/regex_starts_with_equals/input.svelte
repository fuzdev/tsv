<script>
	// Regex starting with = (looks like /= operator but is regex)
	const pattern = /=\s*(?:"[^"]*"|'[^']*'|[^\s'">=]+)/;

	// Simpler case: regex starting with =
	const simple = /=+/;

	// Regex starting with == (double equals)
	const doubleEquals = /==|!=/;

	// Regex starting with => (arrow-like)
	const arrow = /=>\s*/;

	// Division assignment should still work
	let x = 10;
	x /= 2;
</script>
