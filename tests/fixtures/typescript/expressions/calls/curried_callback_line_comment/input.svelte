<script lang="ts">
	// Line comment after first arg - expands correctly
	fn()(
		'name', // comment
		() => {
			fn();
		},
	);

	// Line comment before callback
	fn()(
		'name',
		// leading comment
		() => {
			fn();
		},
	);
</script>
