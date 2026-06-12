<script>
	// Comment between `if` and `(` — tsv keeps it before the paren.
	if/* c */(a){b();}

	// Comment text containing `(` must not be mistaken for the condition's open paren.
	if/* (note) */(a){b();}
</script>
