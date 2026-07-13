<script>
	// non-block, non-inline consequent stays inline (adjustClause)
	if (c) while (x) fn();

	// non-block, non-inline else clause should also stay inline
	if (c) fn();
	else while (x) g();

	// else-for likewise
	if (c) fn();
	else for (;;) g();
</script>
