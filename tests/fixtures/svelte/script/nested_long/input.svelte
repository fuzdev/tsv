<!-- statements inside a nested script measure from their rendered column -->
<div>
	<script>
		// 100 columns: stays on one line
		fn(argument1, argument2, argument3, argument4, argument5, argument6, argument7, aaaaaaaaaaaaaa);
		// 101 columns: the arguments break
		fn(
			argument1,
			argument2,
			argument3,
			argument4,
			argument5,
			argument6,
			argument7,
			aaaaaaaaaaaaaaa
		);
	</script>
</div>

<div>
	<section>
		<script>
			// 100 columns: stays on one line
			fn(argument1, argument2, argument3, argument4, argument5, argument6, argument7, aaaaaaaaaaaa);
			// 101 columns: the arguments break
			fn(
				argument1,
				argument2,
				argument3,
				argument4,
				argument5,
				argument6,
				argument7,
				aaaaaaaaaaaaa
			);
		</script>
	</section>
</div>
