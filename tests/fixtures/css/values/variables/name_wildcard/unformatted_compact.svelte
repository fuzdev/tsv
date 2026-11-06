<style>
	@theme{
		/* namespace reset */
		--*:initial;
		--a-*:initial;

		/* wildcard mid-name and doubled */
		--a-*-b:red;
		--a*:red;
		--a-**:red;
		--*-a:red;
	}

	div{
		--a-*:red;
		--A-*:blue;
	}
</style>
