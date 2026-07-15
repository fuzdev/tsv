<script lang="ts">
	// Line comment before the `.` — stays there, and `.target` continues one level down
	function f() {
		return new.target; // c1
	}

	// The same gap on `import.meta`
	const a =
		import.meta // c2
		.url;

	// Line comment after the `.`
	function g() {
		return new.target; // c3
	}
</script>
