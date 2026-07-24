<script lang="ts">
	// a glued directive freezing a multi-line type forces the parameter list open;
	// the frozen slice keeps its verbatim layout
	function fn(a: /* prettier-ignore */ {
			x:   1;
		}, b: c1) {}
</script>
