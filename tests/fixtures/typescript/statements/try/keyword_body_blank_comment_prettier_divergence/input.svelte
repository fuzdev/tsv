<script>
	// Try - own-line comment sits directly below the keyword, no blank above it
	try
	// c1
	{
	} catch (e) {}

	// Try - a blank between two own-line comments is preserved
	try
	// c2

	// c3
	{
	} catch (e) {}

	// Bare catch - same blank handling after the keyword
	try {
	} catch
	// c4

	// c5
	{}

	// Finally - same blank handling after the keyword
	try {
	} finally
	// c6

	// c7
	{
	}
</script>
