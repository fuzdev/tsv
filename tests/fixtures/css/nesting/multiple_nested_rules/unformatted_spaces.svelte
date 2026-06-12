<style>
	/* Multiple nested rules at same level */
	.class1  {
		color:  red;

		&  .class2  {
			color:  blue;
		}

		&  .class3  {
			color:  green;
		}
	}
</style>
