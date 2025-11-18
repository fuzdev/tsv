<style>
	.card  {
		padding:  1rem;

		/* Modern pseudo-class in nesting */
		&:user-valid  {
			border-color:  green;
		}

		/* Modern pseudo-element in nesting */
		&::highlight(  search  )  {
			background:  yellow;
		}

		/* At-rule nesting with modern features */
		@media  (  width  >=  768px  )  {
			&  >  .title  {
				font-size:  2rem;
			}
		}
	}
</style>
