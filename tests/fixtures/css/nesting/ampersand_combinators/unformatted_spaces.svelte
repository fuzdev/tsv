<style>
	.class1  {
		padding:  10px;

		&  >  .direct-child  {
			margin:  5px;
		}

		&  +  .next-sibling  {
			margin-top:  20px;
		}

		&  ~  .subsequent-sibling  {
			color:  gray;
		}
	}
</style>
