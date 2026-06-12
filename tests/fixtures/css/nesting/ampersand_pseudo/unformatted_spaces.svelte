<style>
	.class1  {
		background:  white;

		&:hover  {
			background:  gray;
		}

		&:focus  {
			outline:  2px  solid  blue;
		}

		&::before  {
			content:  '>';
		}

		&::after  {
			content:  '<';
		}
	}
</style>
