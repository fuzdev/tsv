<script lang="ts">
	// A comment between `async` and the parameters can only be a single-line block
	// comment — every other kind carries a line terminator, which the
	// `async [no LineTerminator here] ArrowFormalParameters` rule rejects (the
	// `async_line_break` fixture covers those rejections).
	const a = async () => /* c */ {};
	const b = async (/* c */ x) => x;
	const c = async /* c */ <T extends A>(x: T) => x;

	// a run stays in order, in the gap the author wrote it in
	const d = async () => /* c1 */ /* c2 */ {};
</script>
