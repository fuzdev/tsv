<script lang="ts">
	// Optional-chained test callees are NOT kept on one line — they wrap like a
	// normal call when long (prettier excludes optional calls from isTestCall).
	describe?.(
		'does something really long and complicated so I have to write a long test name',
		() => {}
	);

	// Short optional call still fits on one line.
	it?.('short', () => {});
</script>
