<style>
	.modern  {
		display:  flex;

		@supports  (  display:  grid  )  {
			display:  grid;
		}

		@supports  (  gap:  1rem  )  {
			gap:  1rem;
		}

		@layer  utilities  {
			margin:  0;
		}
	}
</style>
