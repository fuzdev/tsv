<script lang="ts">
	// a block-comment run below `=>`, in a last and a lone ternary-body arrow argument, at the
	// two widths where the run plus the body would just fit on the `=>` line
	fn('tttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) => /* c */ /* d */ a ? bb : cc);
	fn('ttttttttttttttttttttttttttttttttttttttttttttttttttttttt', (a) => /* c */ /* d */ a ? bb : cc);
	fn((atttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) => /* c */ /* d */ a ? bb : cc);
	fn((attttttttttttttttttttttttttttttttttttttttttttttttttttttttttt) => /* c */ /* d */ a ? bb : cc);
</script>
