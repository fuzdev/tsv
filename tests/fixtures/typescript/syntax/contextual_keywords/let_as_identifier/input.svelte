<script lang="ts">
	// let works in property positions only
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
