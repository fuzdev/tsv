<script>
	// Try - line comment trailing the keyword stays trailing
	try // comment
	{}catch(e){}

	// Try - line comment on its own line keeps its own line
	try
	// comment
	{}catch(e){}

	// Catch with param - line comment trailing ) stays trailing
	try{}catch(e) // comment
	{}

	// Catch with param - line comment on its own line keeps its own line
	try{}catch(e)
	// comment
	{}

	// Bare catch - line comment trailing the keyword stays trailing
	try{}catch // comment
	{}

	// Bare catch - line comment on its own line keeps its own line
	try{}catch
	// comment
	{}

	// Finally - line comment trailing the keyword stays trailing
	try{}finally // comment
	{}

	// Finally - line comment on its own line keeps its own line
	try{}finally
	// comment
	{}
</script>
