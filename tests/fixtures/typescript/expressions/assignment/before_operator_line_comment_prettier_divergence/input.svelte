<script>
	// A line comment before the operator stays after the target; the operator and
	// value drop to a continuation line indented one level.
	a // c
		= 1;

	// Two comments around the statement stay distinct.
	b // c1
		= 2; // c2

	// A member target answers the same way.
	o.p // c
		= 3;

	// So does a compound operator.
	c // c
		+= 4;
</script>
