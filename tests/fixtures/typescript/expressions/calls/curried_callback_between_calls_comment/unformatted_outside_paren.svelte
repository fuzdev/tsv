<script lang="ts">
	// Comment between curried calls (moved inside by prettier)
	fn() /* between */ ('name', () => {
		fn();
	});

	// Comment after args of first call, before second call
	fn('a') /* comment */ ('b', () => {
		fn();
	});
</script>
