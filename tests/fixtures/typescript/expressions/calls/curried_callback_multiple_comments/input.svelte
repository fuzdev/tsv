<script lang="ts">
	// Comment between name and comma only - stays hugged
	fn()('name' /* c1 */, () => {
		fn();
	});

	// Comment after comma only - expands
	fn()(
		'name',
		/* c2 */ () => {
			fn();
		},
	);

	// Both comments - expands (comment after comma)
	fn()(
		'name' /* c1 */,
		/* c2 */ () => {
			fn();
		},
	);
</script>
