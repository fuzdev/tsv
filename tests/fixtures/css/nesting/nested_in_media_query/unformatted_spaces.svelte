<style>
	/* Nested rules inside @media blocks */
	@media  (  min-width:  768px  )  {
		.class1  {
			color:  red;

			&  .class2  {
				color:  blue;
			}
		}
	}
</style>
