<script lang="ts">
	// a multi-line frozen member keeps the union broken one member per line (tsv must-break);
	// prettier instead glues the next member onto the frozen slice's last line
	type U =
		// prettier-ignore
		{
				a:   1
				b: 2
		  } | (b1 & b2);
</script>
