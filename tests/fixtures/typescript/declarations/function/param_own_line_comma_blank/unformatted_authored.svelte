<script>
	// a blank line counts only when it starts on the line the param ends on, so a comma the author
	// pushed onto its own line takes the following blank out of the gap - matching prettier
	function f1(a
,

b) {}

	// authored the other way round, the blank does start on that line and is preserved
	function f2(a

, b) {}
</script>
