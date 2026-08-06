<script>
	// a line comment left behind by a spread's stripped parens runs to end of line, so a
	// line comment written after the `)` takes its own line — on an argument the list
	// continues past, that line sits ahead of the next argument
	fn(...(b // i
	) // t
	, c);

	new A(...(b // i
	) // t
	, c);

	obj.method(...(b // i
	) // t
	, c);
</script>
