<script>
// Short curried chain as a call argument - stays fully inline
fn('x',(a)=>(b)=>(c)=>a+b+c);

// Call expands its args; heads fit together on the argument line (exactly 100), block body opens
fn('first',(argument1)=>(argument2)=>(argument3)=>(argument4)=>(argument5)=>(argument6xxxxx)=>{return argument1;});

// One char over (101): chain heads progressive-indent (first head stays, rest indent one level)
fn('first',(argument1)=>(argument2)=>(argument3)=>(argument4)=>(argument5)=>(argument6xxxxxx)=>{return argument1;});

// Many heads with an object body terminal - clearly progressive indent
fn('first',(argument1)=>(argument2)=>(argument3)=>(argument4)=>(argument5)=>(argument6)=>(argument7)=>(argument8)=>({foo:argument1}));

// Lone argument: the head signatures fit on one line (exactly 100), only the body drops
fn((argument1)=>(argument2)=>(argument3)=>(argument4)=>(argument5)=>(argument6xxxxxxx)=>body);

// One char over (101): the lone argument's heads progressive-indent too
fn((argument1)=>(argument2)=>(argument3)=>(argument4)=>(argument5)=>(argument6xxxxxxxx)=>body);

// The first head's own parameter list breaks - the later heads still take their own lines
fn((argument1111111,argument2222222,argument3333333,argument4444444,argument5555555,argument6666666)=>(argument7)=>body);

// A preceding argument plus a long first head - the chain still progressive-indents
fn('first',(argument1111111,argument2222222,argument3333333,argument4444444,argument5555)=>(argument6)=>body);

// Block body terminal on a lone argument - the block opens on the last head's line
fn((argument1111111,argument2222222,argument3333333,argument4444444,argument5555555)=>(argument6)=>{return body;});

// An authored blank line between arguments is kept; the chain still progressive-indents
fn(aaa,

(argument1111111,argument2222222,argument3333333,argument4444444,argument5555555)=>(argument6)=>body);

// An own-line comment leading the lone argument expands the call; the chain is unaffected
fn(
// c
(argument1111111,argument2222222,argument3333333,argument4444444,argument5555555)=>(argument6)=>body);

// A trailing block comment after the last argument - same progressive indent
fn(aaa,(argument1111111,argument2222222,argument3333333,argument4444444,argument55)=>(argument6)=>body/* c */);

// A typed outer chain does not suppress a nested call's own chain layout - prettier's
// expandLastArg applies to the argument being printed, never to what is nested in it
fn((a=1)=>(b)=>{return g((argument1111111,argument2222222,argument3333333,argument4444444,argument5)=>(argument6)=>body);});
</script>
