<script lang="ts">
	// 100 chars at 1-tab: entire statement fits on one line
	const a = items.map((x) => `${valueeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee}`);

	// 101 chars at 1-tab: map call breaks, arrow body stays inline
	const b = items.map(
		(x) => `${valueeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee}`
	);

	// 100 chars at 2-tab: map call broken, arrow body at boundary
	const c = items.map(
		(x) => `${valueeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee}`
	);

	// 101 chars at 2-tab: arrow breaks to new line
	const d = items.map(
		(x) =>
			`${valueeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee}`
	);
</script>
