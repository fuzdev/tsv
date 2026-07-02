<style>
	/* comma gap - the comment joins the selector line, interior verbatim */
	.class1,  /* x1
y1
z1 */  .class2 {
		color:red;
	}

	/* before opening brace */
	.class3  /* x2
y2 */  {
		color:blue;
	}

	/* declaration value */
	div {
		color:  /* x3
y3 */  red;
	}

	/* between rules - a continuation line may itself start with a comment opener */
	/* x4
/* y4
z4 */
	span {
		color:green;
	}
</style>
