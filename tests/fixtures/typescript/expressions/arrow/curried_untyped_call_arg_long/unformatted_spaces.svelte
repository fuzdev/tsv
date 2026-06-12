<script>
	// Short curried chain as a call argument - stays fully inline
	fn('x',   (a) =>  (b) =>  (c) =>   a + b + c);

	// Call expands its args; heads fit together on the argument line (exactly 100), block body opens
	fn('first',   (argument1) =>  (argument2) =>  (argument3) =>  (argument4) =>  (argument5) =>  (argument6xxxxx) =>  {  return argument1;  });

	// One char over (101): chain heads progressive-indent (first head stays, rest indent one level)
	fn('first',   (argument1) =>  (argument2) =>  (argument3) =>  (argument4) =>  (argument5) =>  (argument6xxxxxx) =>  {  return argument1;  });

	// Many heads with an object body terminal - clearly progressive indent
	fn('first',   (argument1) =>  (argument2) =>  (argument3) =>  (argument4) =>  (argument5) =>  (argument6) =>  (argument7) =>  (argument8) =>  ({  foo: argument1  }));
</script>
