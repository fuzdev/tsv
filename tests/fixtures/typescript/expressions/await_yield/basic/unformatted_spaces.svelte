<script>
	// await
	await  promise;
	await   fetch(url);
	await obj.method(  );
	await  arr[ 0 ]();
	await a .b. c.d();
	await  ( x + y );

	// yield (in generator)
	function* gen() {
		yield  value;
		yield   fn();
		yield obj.method(  );
		yield  arr[ 0 ]();
		yield a .b. c.d();
		yield  ( x + y );
		yield*  iterable;
	}
</script>
