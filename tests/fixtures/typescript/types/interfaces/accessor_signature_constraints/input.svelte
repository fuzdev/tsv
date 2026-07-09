<script lang="ts">
	interface I {
		// a getter signature may carry a return type; a setter takes one plain param — valid
		get a(): number;
		set a(v: number);
	}
</script>
