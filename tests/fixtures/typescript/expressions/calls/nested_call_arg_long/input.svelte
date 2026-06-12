<script lang="ts">
	// 100 chars - fits on one line
	fn1(a).methodAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA('value1 value2 value3 value4', fn2(x));

	// 101 chars - breaks outer call, nested call stays inline
	fn1(a).methodAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA(
		'value1 value2 value3 value4',
		fn2(x),
	);
</script>
