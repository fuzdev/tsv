<script lang="ts">
	// 100 chars on line (at boundary)
	const a = items
		.filter(
			aa + bb + cc + dd + ee + ff + gg + hh + ii + jj + kk + ll + mm + nn + oo + pp + qq + rr + sss
		)
		.join();

	// 101 chars - exceeds, so wraps
	const b = items
		.filter(
			aa + bb + cc + dd + ee + ff + gg + hh + ii + jj + kk + ll + mm + nn + oo + pp + qq + rr + ssss
		)
		.join();
</script>
