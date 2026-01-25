<script>
	switch (x) {
		// inline after colon
		case 'a': { // comment
			fn();
			break;
		}
		// own line between label and block
		case 'b': // comment
		{
			fn();
			break;
		}
	}
</script>
