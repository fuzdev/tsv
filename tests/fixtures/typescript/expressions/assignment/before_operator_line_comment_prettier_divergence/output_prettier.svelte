<script>
	// A line comment before the operator stays after the target; the operator and
	// value drop to a continuation line indented one level.
	a = 1; // c

	// Two comments around the statement stay distinct.
	b = 2; // c1 // c2

	// A member target answers the same way.
	o.p = 3; // c

	// So does a compound operator.
	c += 4; // c
</script>
