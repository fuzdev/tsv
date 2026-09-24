<script>
	// a line comment left behind by a rest element's stripped parens runs to end of line,
	// so a line comment written after the `)` takes its own line — in an array and in an
	// object assignment pattern
	[...(a // c1
	) // c2
	] = x;

	[b, ...(a // c1
	) // c2
	] = x;

	({...(a // c1
	) // c2
	} = x);

	({b, ...(a // c1
	) // c2
	} = x);
</script>
