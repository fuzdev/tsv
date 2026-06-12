<style>
	/* Basic :nth-col() - select specific columns */
	td:nth-col(1){font-weight:bold;}

	td:nth-col(3){text-align:right;}

	/* Even/odd columns */
	td:nth-col(even){background:#f0f0f0;}

	td:nth-col(odd){background:white;}

	/* An+B formulas */
	td:nth-col(2n){background:gray;}

	td:nth-col(2n+1){background:lightgray;}

	td:nth-col(3n){border-left:2px solid black;}

	td:nth-col(3n+2){color:blue;}

	/* Negative formulas - first N columns */
	td:nth-col(-n+3){font-weight:bold;}

	/* :nth-last-col() - count from end */
	td:nth-last-col(1){text-align:right;}

	td:nth-last-col(2){font-style:italic;}

	/* Even/odd from end */
	td:nth-last-col(even){background:#e0e0e0;}

	td:nth-last-col(odd){background:#f5f5f5;}

	/* An+B formulas from end */
	td:nth-last-col(2n){border-right:1px solid blue;}

	td:nth-last-col(3n+1){color:green;}

	/* Negative formulas from end - last N columns */
	td:nth-last-col(-n+2){font-weight:bold;}

	/* Combined with other selectors */
	table.class1 td:nth-col(1){text-align:left;}

	tr.class2 td:nth-col(2n){background:yellow;}

	/* With type selectors */
	th:nth-col(1){text-align:left;}

	th:nth-last-col(1){text-align:right;}

	/* Multiple pseudo-classes */
	td:nth-col(2):hover{background:lightblue;}

	td:nth-last-col(1):focus{outline:2px solid red;}

	/* Edge cases - zero offset */
	td:nth-col(0n+1){border-left:1px solid black;}

	/* With attribute selectors */
	td[data-type='number']:nth-col(2n){text-align:right;}

	/* Complex selectors */
	tbody > tr > td:nth-col(1){font-weight:bold;}

	/* Combined :nth-col and :nth-last-col */
	td:nth-col(2):nth-last-col(3){background:orange;}
</style>
