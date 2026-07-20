<script>
	// a blank line counts only when it starts on the line the argument ends on, so a comma the
	// author pushed onto its own line takes the following blank out of the gap - matching prettier
	fn(argument1
,

argument2);

	// authored the other way round, the blank does start on that line and is preserved
	fn(argument1

, argument2);
</script>
