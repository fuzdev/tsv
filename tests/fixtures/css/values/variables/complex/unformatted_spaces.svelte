<style>
	.class1 {
		/* var() in calc() */
		width: calc(  var(  --var1  )  *  2  );
		height: calc(  var(--var1)  *  2  +  10px  );
		margin: calc(  var(--var2)  -  5px  );
		padding: calc(  var(  --var3  )  +  var(  --var4  )  );

		/* var() in color functions */
		color: rgb(  var(--r)  var(--g)  var(--b)  );
		background: rgba(  var(  --r  )  var(  --g  )  var(--b)  /  var(  --alpha  )  );
		border-color: hsl(  var(--h)  var(  --s  )  var(--l)  );

		/* var() in functions with fallbacks in calc() */
		width: calc(  var(  --w,  100%  )  -  2rem  );
		margin: calc(  var(  --var1,  1rem  )  *  2  +  var(  --var5,  0px  )  );

		/* Complex composition */
		box-shadow: var(  --x,  0  )  var(--y, 2px)  var(  --blur,  4px  )  var(  --color,  rgba(  0,  0,  0,  0.1  )  );
		transform: translate(  var(  --tx  ),  var(--ty)  )  rotate(  var(  --angle,  0deg  )  );

		/* var() in gradients */
		background: linear-gradient(  var(  --dir,  to right  ),  var(  --a  ),  var(  --b  )  );
	}
</style>
