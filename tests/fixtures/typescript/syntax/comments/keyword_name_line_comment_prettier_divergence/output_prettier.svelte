<script lang="ts">
	// A line comment in a declaration's keyword-to-name gap indents the
	// continuation one level; prettier keeps it flat.
	function // c
	f1() {}

	async function // c
	f2() {}

	function* // c
	g1() {}

	class // c
	C1 {}

	abstract class // c
	C2 {}

	enum // c
	E1 {}

	const enum // c
	E2 {}

	declare function // c
	f3(): void;
</script>
