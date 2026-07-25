<script lang="ts">
	// a glued directive is inert — the inline parameter list formats normally
	function a(p: T, /* prettier-ignore */ q: { x: 1 }, r: U) {}

	// signature-side parameter lists classify identically
	type B = (p: T, /* prettier-ignore */ q: { x: 1 }) => void;

	// a multi-line parameter keeps its authored break; the list breaks by ordinary layout
	function c(p: T, /* prettier-ignore */ q: {
			x: 1;
		}, r: U) {}
</script>
