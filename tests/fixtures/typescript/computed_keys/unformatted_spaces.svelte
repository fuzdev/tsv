<script>
	// Computed keys with different expression types
	const obj = {
		[ key ]: value,
		[ "literal" ]: 1,
		[ 1+2 ]: 3,
		[ obj.prop ]: 4,
		[ fn() ]: 5,
		[ a?b:c ]: 6,
	};

	// Inline computed keys
	const inline = { [ x ]:1,[ y ]:2 };

	// Mixed regular and computed keys
	const mixed = {
		regular: 1,
		[ computed ]: 2,
		"string-key": 3,
		[ expr+1 ]: 4,
	};
</script>
