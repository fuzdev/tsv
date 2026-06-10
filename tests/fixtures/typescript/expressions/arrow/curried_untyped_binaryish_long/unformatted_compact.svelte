<script>
// Short curried chain as a logical-expression operand - stays fully inline
const short=cond||((a)=>(b)=>(c)=>a+b+c);

// Heads fit together on the operand line (exactly 100), simple body hangs on the next line
const fits=condition??((argument1)=>(argument2)=>(argument3)=>(argument4)=>(argument5)=>(argument6xxxxxx)=>body);

// One char over (101): chain heads progressive-indent (first head stays, rest indent one level)
const broken=condition??((argument1)=>(argument2)=>(argument3)=>(argument4)=>(argument5)=>(argument6xxxxxxx)=>body);
</script>
