<script lang="ts">
	type A = {
		a: A /* block */;
		b /* before colon */: B;
		c: /* after colon */ C;
	};
</script>
