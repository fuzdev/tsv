<script>
	a.b.items.filter(
		(item) =>
			item.a !== 'value1' && item.b !== 'value2' && (item.c !== 'value3' || item.d.trim() !== '')
	).length > 1;
</script>
