<script>
	// Two args: string + arrow
	fn('a',(x)=>{return y});

	// Two args: identifier + arrow
	const a=fn(b,(x)=>{return y});

	// Three args: strings + arrow
	fn('a','b',(x)=>{return y});

	// Four args: strings + arrow
	fn('a','b','c',(x)=>{return y});
</script>
