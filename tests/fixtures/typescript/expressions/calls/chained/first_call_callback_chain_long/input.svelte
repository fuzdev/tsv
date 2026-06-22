<script>
	function a() {
		if (cond) {
			// 100 chars: .map((file) => fits on same line, arrow body wraps
			const abcde = fn_name__(directory, (file) => extensions.includes(extname(file))).map((file) =>
				resolve(directory, file)
			);

			// 101 chars: .map( breaks, first call args stay inline, callback on next line
			const abcdef = fn_name__(directory, (file) => extensions.includes(extname(file))).map(
				(file) => resolve(directory, file)
			);
		}
	}
</script>
