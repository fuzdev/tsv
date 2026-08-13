<script>
	// Short curried chain as a chain call's argument - stays fully inline
	obj.method('x',   (a) =>  (b) =>  (c) =>   a + b + c).next();

	// The head signatures fit on the argument line (exactly 100), only the body drops
	obj.method(   (argument1) =>  (argument2) =>  (argument3) =>  (argument4) =>  (argument5) =>  (argument6xxxxx) =>   body).next();

	// One char over (101): the heads progressive-indent (first head stays, rest indent one level)
	obj.method(   (argument1) =>  (argument2) =>  (argument3) =>  (argument4) =>  (argument5) =>  (argument6xxxxxx) =>   body).next();

	// A preceding argument plus a long first head - the chain still progressive-indents
	obj.method('first',   (argument1111111,   argument2222222,   argument3333333,   argument4444444,   argument5) =>  (argument6) =>   body).next();

	// Block body terminal - the argument breaks out rather than hugging `.method(`
	obj.method(   (argument1111111,   argument2222222,   argument3333333,   argument4444444,   argument5) =>  (argument6) =>  {  return body;  }).next();

	// Object body terminal - same break-out
	obj.method(   (argument1111111,   argument2222222,   argument3333333,   argument4444444,   argument5) =>  (argument6) =>  ({  a: body  })).next();

	// A short chain argument still hugs `.method(` with its block open
	obj.method(   (a) =>  (b) =>  {  return body;  }).next();

	// A trailing block comment defeats the hug - the argument breaks out (a `function`
	// expression is the other shape that reaches the same arm)
	obj.method(   (a) =>  (b) =>  {  return body;  }   /* c */).next();
	obj.method(   function () {  return body;  }   /* c */).next();
</script>
