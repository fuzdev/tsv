<script>
	/* Short chain - stays inline (under 100 chars) */
	const short = obj1.prop1.prop2.prop3;

	/* Medium chain - assignment wraps, chain stays inline */
	const medium =
		obj1.prop1.prop2.prop3.prop4.prop5.prop6.prop7.prop8.prop9.prop10.prop11.prop12.prop13.prop14;

	/* Long chain - assignment wraps AND chain breaks at dots */
	const long =
		obj1.prop1.prop2.prop3.prop4.prop5.prop6.prop7.prop8.prop9.prop10.prop11.prop12.prop13.prop14
			.prop15.prop16;
</script>
