<script lang="ts">
	// A multi-line block ending a redundant pair around a sequence's last operand, ahead of a
	// non-null `!`, keeps a pair around the sequence and the block, as the bare spelling does.
	x = (a, b /* x1
	 */)!;

	x = (a, b /* x2
	 */)![0];

	x = (a, b /* x3
	 */)!.m;

	x = (a, b /* x4
	 */)!();

	(a, b /* x5
	 */)!;
</script>

<div title={(a, b /* x6
 */)!}></div>

{(a, b /* x7
 */)!.m}
