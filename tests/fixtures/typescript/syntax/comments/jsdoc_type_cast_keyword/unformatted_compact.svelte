<script>
// await with JSDoc cast
async function fn1(){const a=await /** @type {Promise<A>} */(expr);}

// yield with JSDoc cast
function* fn2(){const b=yield /** @type {A} */(expr);}
</script>
