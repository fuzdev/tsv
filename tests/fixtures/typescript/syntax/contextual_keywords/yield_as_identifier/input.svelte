<script lang="ts">
	// yield works in property positions only
	const obj = { yield: 'value' };

	const obj2 = {
		yield() {
			return 'a';
		}
	};

	{
		const { yield: y } = obj;
	}

	// Inside generators, yield is the operator
	function* gen() {
		yield 'a';
	}
</script>
