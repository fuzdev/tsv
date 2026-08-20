<style>
	/* a glued run stays glued: a space between the two comments would be a
	   <whitespace-token>, which selectors-4 forbids inside a <wq-name> */
	svg/* c1 *//* c2 */|rect
	{
		color: red;
	}

	/* the attribute spelling of the same wq-name */
	[svg/* c3 *//* c4 */|attr]
	{
		color: blue;
	}

	/* and the <attr-matcher>'s own interior, whitespace-forbidden for the same reason */
	[attr~/* c5 *//* c6 */='value']
	{
		color: green;
	}
</style>
