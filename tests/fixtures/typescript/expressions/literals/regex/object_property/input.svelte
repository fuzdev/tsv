<script>
	// regex as the sole property value
	const obj1 = {a: /re/};

	// regex value followed by a sibling property
	const obj2 = {a: /re/, b: 2};

	// regex with flags as the last property value
	const obj3 = {a: 1, b: /re/g};

	// regex value under a computed key
	const obj4 = {[a]: /re/};

	// bare regex spread (SpreadElement span)
	const obj5 = {.../re/};
</script>
