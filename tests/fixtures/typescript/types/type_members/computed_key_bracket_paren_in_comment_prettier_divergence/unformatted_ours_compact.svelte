<script lang="ts">
	interface I {
		[x]/* a(b) */: number;
		[x]/* a(b) */(): void;
		get [x]/* a(b) */(): number;
		set [x]/* a(b) */(a: number);
	}
</script>
