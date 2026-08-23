<script lang="ts">
	// A decorator-first `export` is the last statement in the Program body, and its
	// wire node starts at `export` while the statement starts at the `@` — the
	// trailing run after it still attaches to the export, not to the Program.
	@dec
	export class C {}
	// t1
	// t2
</script>
