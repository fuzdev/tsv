<script lang="ts">
	// Block comment after comma - leading on callback
	fn()(
		'name',
		/* comment */ () => {
			fn();
		},
	);
</script>
