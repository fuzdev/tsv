<script>
	// Object-pattern target with an object value: `= {` lands on column 100, the value hugs the `=`
	(  {  aaaaaaaaa1,  aaaaaaaaa2,  aaaaaaaaa3,  aaaaaaaaa4,  aaaaaaaaa5,  aaaaaaaaa6,  aaaaaaaaaaaaaaaa7  }  =  {
		aaaaaaaaa1:  1,
		aaaaaaaaa2:  2,
		aaaaaaaaa3:  3,
		aaaaaaaaa4:  4,
		aaaaaaaaa5:  5,
		aaaaaaaaa6:  6,
		aaaaaaaaaaaaaaaa7:  7
	}  );

	// One column more: the target stays flat and the value drops after the `=` (101 visual)
	(  {  bbbbbbbbb1,  bbbbbbbbb2,  bbbbbbbbb3,  bbbbbbbbb4,  bbbbbbbbb5,  bbbbbbbbb6,  bbbbbbbbbbbbbbbbb7  }  =
		{
			bbbbbbbbb1:  1,
			bbbbbbbbb2:  2,
			bbbbbbbbb3:  3,
			bbbbbbbbb4:  4,
			bbbbbbbbb5:  5,
			bbbbbbbbb6:  6,
			bbbbbbbbbbbbbbbbb7:  7
		}  );
</script>
