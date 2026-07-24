<script lang="ts">
	// a glued directive freezing a multi-line member breaks the tuple one member per
	// line (tsv must-break); prettier instead keeps the container flat, gluing the
	// separators around the verbatim slice
	type T = [a, /* prettier-ignore */ {
			x:   1
		}, b];
</script>
