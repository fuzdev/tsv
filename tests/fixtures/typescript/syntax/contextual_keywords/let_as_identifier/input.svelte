<script lang="ts">
	// let in property positions; it is a binding name too, but acorn rejects that
	// and so cannot be a fixture — see tests/strict_reserved_word_as_name.rs
	const obj = { let: 'value' };

	const a = obj.let;

	const obj2 = {
		let() {
			return 'a';
		}
	};

	{
		const { let: l } = obj;
	}
</script>
