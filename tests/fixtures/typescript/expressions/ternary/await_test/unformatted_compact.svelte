<script>
// Await in test position — needs parens for clarity
const a=(await b(c))?d:e;

// Await with member expression
const f=(await a.b(c))?d:e;

// Simple await
const g=(await a)?b:c;

// Short control — fits inline, no parens needed on non-await
const h=cond?a:b;
</script>
