<script>
// Short chains (2 segments) - not treated as chains per Prettier
a=b=1;
x=y='short';

// Long chains (3+ segments) - chain formatting
a=b=c=1;
x=y=z=w='value';

// Chain with long values
longVariableName=anotherLongName=yetAnotherLongVariableName='some long string value here';

// Chain with function call
a=b=c=getSomeValue();

// Chain with arrow function (chain-tail-arrow-chain case)
const f=(g=(x)=>(y)=>x+y);

// Chain with object
obj1=obj2={key:'value'};

// Chain exceeding line width
veryLongVariableNameHere=anotherVeryLongVariableName=yetAnotherVeryLongVariable=someReallyLongFunctionCall();

// Mixed chain with different expression types
a=b=c=d=obj.prop.method().anotherMethod();

// Variable declaration chains
const v1=(v2=v3=1);
let l1=(l2=l3='string');

// Expression statement chains
(()=>{
a=b=c=1;
})();

// Nested in object
const nested={
prop:(a=b=c=1),
};
</script>
