<script>
	// A multiline block the author broke after keeps the break: the operator and
	// value drop to a continuation line indented one level.
	d =
		/* x
y */
		5;

	// One whose operator shares its closing line stays glued.
	e /* x
y */ = 6;
</script>
