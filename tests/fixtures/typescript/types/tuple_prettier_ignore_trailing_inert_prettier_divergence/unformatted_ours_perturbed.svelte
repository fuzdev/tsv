<script lang="ts">
	// a trailing directive is inert: tsv honors a directive only where it precedes
	// the node, so both members reformat normally (prettier instead freezes the
	// preceding member backward)
	type T = [
		{ x:   1 }, // prettier-ignore
		{ y:   2 }
	];
</script>
