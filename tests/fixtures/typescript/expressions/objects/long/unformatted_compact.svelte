<script>
	/* short - stays inline (under 100 chars) */
	const short={prop1:'val1', prop2:'val2', prop3:'val3'};
	/* at boundary - exactly 100 chars (stays inline) */
	const boundary={aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1};
	/* exceeds boundary - 101 chars (wraps) */
	const long={aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:1};
	/* multiple properties - wraps (one property per line) */
	const multi={property1:'value1',property2:'value2',property3:'value3',property4:'value4',property5:'value5'};
</script>
