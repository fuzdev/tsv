<script lang="ts">
	// a line comment runs to end of line, so the argument list drops to a
	// continuation line
	fn // c1
		?.(a);

	// member callee — the chain stays flat
	obj.m /* c2 */?.(a);

	// both differences in one shape
	obj.m // c3
		?.(a);
</script>
