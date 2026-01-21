<script>
	let a, b, c, x;
</script>

<!-- Assignment expressions need parens in expression tags -->
{(a = b)}
{(a += b)}
{(a ||= b)}

<!-- Multiple expression tags -->
{(a = b)}{(c = x)}

<!-- With text -->
prefix {(a = b)} suffix
