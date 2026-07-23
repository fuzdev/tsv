<script lang="ts">
	type X100 = { a: A } & ({ bb: BB } & { cc: CC } & { dd: D; ee: E; ff: F; gg: GGGGGGGGGGGGGGGGG });
	type X101 = { a: A } & ({ bb: BB } & { cc: CC } & {
		dd: D;
		ee: E;
		ff: F;
		gg: GGGGGGGGGGGGGGGGGG;
	});
</script>
