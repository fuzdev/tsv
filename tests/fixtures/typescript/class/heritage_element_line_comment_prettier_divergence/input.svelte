<script lang="ts">
	class C
		extends B
		implements
			// leading on first
			Z, // trailing on first
			// leading on second
			Y
	{
		m() {}
	}
</script>
