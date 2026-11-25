<script lang="ts">
	interface Single {
		();
		new ();
	}
	type Literal = { (a): void; new (a) };
</script>
