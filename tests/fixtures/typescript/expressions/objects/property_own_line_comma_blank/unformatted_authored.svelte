<script>
	// a blank line counts only when it starts on the line the property ends on, so a comma the
	// author pushed onto its own line takes the following blank out of the gap - matching prettier
	const a = { property1: 1
,

property2: 2 };

	// authored the other way round, the blank does start on that line and is preserved
	const b = { property1: 1

, property2: 2 };
</script>
