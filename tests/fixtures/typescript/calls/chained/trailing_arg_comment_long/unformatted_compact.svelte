<script>
// Trailing block comment on the hugged callback - 100 chars (stays inline)
f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,1).g((x)=>c()/* c */);

// Trailing block comment - 101 chars (the arguments break out, the comment stays)
f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,1).g((x)=>c()/* c */);

// Ternary body - the arguments break out the same way
f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,1).g((x)=>(a?b:c)/* c */);

// Array body - the arguments break out instead of hugging the brackets
f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,1).g((x)=>[x]/* c */);

// Block body - the arguments break out instead of hugging the braces
f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,1).g((x)=>{c();}/* c */);

// A comment leading the argument breaks the arguments out too
f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,1).g(/* c */(x)=>c());

// Trailing line comment - the arguments break out at any width
a.b(1).g((x)=>c() // c
);

// The same trailing comment without a chain - the arguments break out
fn(1,(x)=>cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc()/* c */);

// No comment - the callback hugs the parens
f(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,1).g((x)=>c());
</script>
