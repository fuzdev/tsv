<script lang="ts">
	// Typed identifier with an object value: `= {` lands on column 100, the value hugs the `=`
	const  aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:  Record<string,  string>  =  {
		a:  1,
		b:  2,
		c:  3,
		d:  4,
		e:  5,
		f:  6,
		g:  7
	};

	// One column more: the type arguments stay flat and the value drops after the `=` (101 visual)
	const  bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:  Record<string,  string>  =
		{  a:  1,  b:  2,  c:  3,  d:  4,  e:  5,  f:  6,  g:  7  };

	// A binding wider than the print width breaks its type arguments, and the value hugs `> = {`
	const  cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc:  Record<
		string,
		string
	>  =  {  a:  1,  b:  2,  c:  3,  d:  4,  e:  5,  f:  6,  g:  7  };
</script>
