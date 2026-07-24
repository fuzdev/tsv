<script lang="ts">
	// a directive with ANY content before it on its physical line is TRAILING → inert: tsv
	// keeps the comment where the author put it and reformats the annotation normally;
	// prettier keeps it trailing the `:` too but freezes the annotation (output_prettier)
	let v: // prettier-ignore
		{ y: 2 } | c1;
</script>
