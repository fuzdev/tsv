<script lang="ts">
	const // c
		x1 = 1;
	let // c
		x2 = 2;
	var // c
		x3 = 3;

	// the broken gap carries the continuation indent, so later declarators sit at
	// one level, not two
	const // c
		x4 = 4,
		x5 = 5;
</script>
