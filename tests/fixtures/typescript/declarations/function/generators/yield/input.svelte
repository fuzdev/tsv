<script>
	// Basic yield
	function* gen() {
		yield value;
		yield;
	}

	// Yield in expression context
	function* gen2() {
		x = yield y;
		const z = yield;
	}

	// Yield with different value types
	function* gen3() {
		yield 1;
		yield 'string';
		yield obj.prop;
		yield fn();
	}

	// Yield in conditional
	function* gen4() {
		if (cond) yield a;
		yield cond ? x : y;
	}
</script>
