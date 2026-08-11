<script lang="ts">
	// async function declaration — `async` and `function` on the same line (valid)
	async function a() {}

	// `AsyncFunctionDeclaration : async [no LineTerminator here] function` (ecma262):
	// a line break demotes `async` to an ordinary identifier reference and ASI splits
	// the statement into an expression statement plus a plain function declaration
	async;
	function b() {}
</script>
