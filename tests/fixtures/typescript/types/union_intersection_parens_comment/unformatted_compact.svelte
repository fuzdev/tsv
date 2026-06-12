<script lang="ts">
	// trailing comment in stripped union paren
	type A1=(a/* c */)|b;

	// leading comment in stripped union paren
	type A2=a|(/* c */b);

	// both leading and trailing comments
	type A3=(/* c1 */a/* c2 */)|b;

	// trailing comment in stripped intersection paren
	type A4=(a/* c */)&b;

	// leading comment in stripped intersection paren
	type A5=a&(/* c */b);

	// both comments in intersection
	type A6=(/* c1 */a/* c2 */)&b;

	// union with multiple members — paren stripped from each
	type A7=(a/* c1 */)|(/* c2 */b)|c;

	// intersection with multiple members — paren stripped from each
	type A8=(a/* c1 */)&(/* c2 */b)&c;
</script>
