<script lang="ts">
	// Line comment before the `.` — stays there, and `.target` continues one level down
	function f() {
		return new // c1
			.target;
	}

	// The same gap on `import.meta`
	const a = import // c2
		.meta.url;

	// Line comment after the `.`
	function g() {
		return new. // c3
			target;
	}
</script>
