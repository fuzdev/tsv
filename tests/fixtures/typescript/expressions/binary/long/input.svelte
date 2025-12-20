<script>
	/* short arithmetic - stays inline */
	const short = 1 + 2 + 3 + 4 + 5;

	/* moderately long - breaks after =, expression stays inline */
	const moderate =
		1 + 2 + 3 + 4 + 5 + 6 + 7 + 8 + 9 + 10 + 11 + 12 + 13 + 14 + 15 + 16 + 17 + 18 + 19 + 20;

	/* very long - breaks after = AND wraps each operand */
	const verylong =
		1 +
		2 +
		3 +
		4 +
		5 +
		6 +
		7 +
		8 +
		9 +
		10 +
		11 +
		12 +
		13 +
		14 +
		15 +
		16 +
		17 +
		18 +
		19 +
		20 +
		21 +
		22 +
		23;

	/* long logical - same behavior */
	const logic =
		a_long_condition_name ||
		another_long_condition ||
		yet_another_condition ||
		one_more_condition ||
		final_condition;

	/* long comparison chain */
	const compare =
		some_value > threshold_one &&
		some_value < threshold_two &&
		other_value >= minimum_value &&
		other_value <= maximum_value;
</script>
