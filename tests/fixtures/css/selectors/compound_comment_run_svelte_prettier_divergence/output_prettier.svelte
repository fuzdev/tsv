<style>
	/* two adjacent glued comments keep the compound (not a descendant) */
	.a/* c1 *//* c2 */.b
	{
		color: red;
	}

	/* a glued comment run mid-compound */
	.a.b/* c3 *//* c4 */.c
	{
		color: blue;
	}
</style>
