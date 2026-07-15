<script lang="ts">
	class C
		extends B
		// leading on first
		implements
			Z, // trailing on first
			// leading on second
			Y
	{
		m() {}
	}
</script>
