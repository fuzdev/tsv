<script lang="ts">
	// async function declaration — `async` and `function` on the same line (valid)
	async function a() {}
	export async function c() {}

	// `AsyncFunctionDeclaration : async [no LineTerminator here] function` (ecma262):
	// a line break demotes `async` to an ordinary identifier reference and ASI splits
	// the statement into an expression statement plus a plain function declaration
	async;
	function b() {}

	// under `export` the same break leaves nothing valid — an export needs a
	// Declaration, and a bare `async` is not one — so it is a syntax error rather
	// than two statements; `input_invalid_export.svelte` pins that rejection
</script>
