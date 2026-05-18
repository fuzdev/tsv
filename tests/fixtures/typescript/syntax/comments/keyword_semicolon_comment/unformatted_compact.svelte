<script>
	// Return with comment before semicolon (no argument)
	function a(){return/* comment */;}

	// Break with block comment before semicolon
	for(;;){break;/* comment */}

	// Continue with block comment before semicolon
	for(;;){continue;/* comment */}

	// Break with line comment before semicolon
	for(;;){break;// comment
	}

	// Continue with line comment before semicolon
	for(;;){continue;// comment
	}

	// Return with line comment before semicolon (no argument)
	function b(){return;// comment
	}
</script>
