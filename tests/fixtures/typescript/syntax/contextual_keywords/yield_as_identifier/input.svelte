<script lang="ts">
	// yield in property positions; it is a binding name too, but acorn rejects that
	// and so cannot be a fixture — see tests/strict_reserved_word_as_name.rs
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
