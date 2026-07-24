<script lang="ts">
	// a directive with ANY content before it on its physical line is TRAILING → inert: tsv
	// keeps the comment where the author put it and reformats the value normally; prettier
	// relocates the directive own-line and freezes the whole value (output_prettier)
	type A = // prettier-ignore
		{x:1} | b;
</script>
