<script lang="ts">
	interface I {
		// comment between ] and : is preserved before the value type
		[k: string] /* c1 */ : number;
	}
	type T = {
		[k: string] /* c2 */ : number;
	};
</script>
