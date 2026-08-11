<script>
	// A comment before a dropped empty statement binds inside its own slot and trails.
	a1(); /* c1 */ ; let b1 = 1;

	// A comment after the dropped empty statement leads the next statement.
	a2(); ; /* c2 */ let b2 = 2;

	// One on each side: the dropped statement splits the gap.
	a3(); /* c3 */ ; /* c4 */ let b3 = 3;
</script>
