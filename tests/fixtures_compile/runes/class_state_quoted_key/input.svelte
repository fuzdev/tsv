<script>
	class Toggle {
		'aria-pressed' = $state(false);
	}
	const t = new Toggle();
</script>

<button aria-pressed={t['aria-pressed']}>x</button>
