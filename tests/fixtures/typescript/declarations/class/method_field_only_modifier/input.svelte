<script lang="ts">
	class C {
		// readonly and declare are valid on fields
		readonly a = 1;
		declare b: number;
		// a plain method carries neither
		m() {}
	}
</script>
